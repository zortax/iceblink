//! The coalescing rules, applied while the emission stream is swept.

mod open;
mod record;

use iceblink::{DeviceRect, PlannedPass, PlannedPath, VectorPlan};

use crate::plan::order::DrawOrder;
use crate::plan::pass::clips::ClipChains;
use crate::plan::pass::overlap::{Intervening, Overlap};
use crate::plan::pass::trap;

pub(crate) use self::open::OpenPath;
pub(crate) use self::record::{Frame, cut, pairwise_disjoint, relative};

use self::open::Open;
use self::record::Recorder;

/// The vector-pass planner of one frame.
///
/// The coalescer takes the emission stream in paint order and applies the rules of the
/// [`pass`](crate::plan::pass) module to it. It receives only the paths the damage cull kept.
#[derive(Debug, Default)]
pub(crate) struct Coalescer {
    open: Open,
    depth: u32,
    recorder: Recorder,
    /// Scratch for the open pass's path orders.
    orders: Vec<DrawOrder>,
    /// Path inks of recorded passes with the composite order a later overlapping instance must
    /// sort above.
    raises: Vec<(DeviceRect, DrawOrder)>,
    /// Scratch for sorting the passes into paint order.
    sorted: Vec<(DrawOrder, u32)>,
    passes: Vec<PlannedPass>,
    items: Vec<PlannedPath>,
}

impl Coalescer {
    /// Forgets the previous frame.
    pub(crate) fn clear(&mut self) {
        self.open.active = false;
        self.depth = 0;
        self.recorder.clear();
        self.raises.clear();
    }

    /// Calls `raise` with every path ink of the passes closed since the last call whose path sorts
    /// below the pass's composite, together with the composite's order.
    ///
    /// An instance drawn later that overlaps such an ink must take an order above the composite.
    pub(crate) fn drain_raises(&mut self, mut raise: impl FnMut(DeviceRect, DrawOrder)) {
        for (ink, order) in self.raises.drain(..) {
            raise(ink, order);
        }
    }

    /// Returns the clip links the residuals add inside rasters.
    pub(crate) fn clip_layers(&self) -> u32 {
        self.recorder.clip_layers
    }

    /// Returns the composite order of each pass, parallel to the plan's passes.
    pub(crate) fn orders(&self) -> &[DrawOrder] {
        &self.recorder.orders
    }

    /// Takes a group boundary, which ends the open pass.
    pub(crate) fn boundary(
        &mut self,
        opens: bool,
        frame: Frame,
        clips: &mut impl ClipChains,
        out: &mut VectorPlan,
    ) {
        self.close(frame, clips, out);
        self.depth = if opens {
            self.depth + 1
        } else {
            self.depth.saturating_sub(1)
        };
    }

    /// Takes a drawn instance that is not a path.
    pub(crate) fn occluder(&mut self, bounds: DeviceRect, overlap: Overlap) {
        let open = &mut self.open;
        if !open.active {
            return;
        }

        let instance = Intervening {
            bounds,
            accumulated: open.inks.len(),
        };
        open.caught = open.caught
            || match overlap {
                Overlap::PerItemInk => open.cells.meets(bounds, &open.inks, |_| true),
                reading => reading.caught(&open.inks, &instance),
            };
        open.intervening.push(instance);
    }

    /// Takes a drawn path of ink `ink` through the chain `clip`, closing the open pass when the
    /// path starts a new one.
    ///
    /// The path is admitted with [`Coalescer::admit`] once it has its order.
    pub(crate) fn prepare(
        &mut self,
        clip: u32,
        ink: DeviceRect,
        overlap: Overlap,
        frame: Frame,
        clips: &mut impl ClipChains,
        out: &mut VectorPlan,
    ) {
        let masked = clips.has_mask(clip);
        let open = &self.open;
        let fresh = !open.active
            || masked
            || open.sealed
            || if overlap.ignores_candidate() {
                open.caught
            } else {
                overlap.splits(&open.inks, ink, &open.intervening)
            };

        if fresh {
            self.close(frame, clips, out);
            self.open.start(self.depth > 0);
        }
        self.open.sealed |= masked;
    }

    /// Admits the path [`Coalescer::prepare`] took to the open pass.
    pub(crate) fn admit(&mut self, path: OpenPath, ink: DeviceRect) {
        self.open.admit(path, ink);
    }

    /// Closes the open pass and puts the passes into paint order.
    pub(crate) fn finish(
        &mut self,
        frame: Frame,
        clips: &mut impl ClipChains,
        out: &mut VectorPlan,
    ) {
        self.close(frame, clips, out);
        self.sort(out);
    }

    /// Records the open pass, one pass per path when its composite would trap an instance.
    fn close(&mut self, frame: Frame, clips: &mut impl ClipChains, out: &mut VectorPlan) {
        let open = &mut self.open;
        if !open.active {
            return;
        }
        open.active = false;

        self.orders.clear();
        self.orders.extend(open.paths.iter().map(|path| path.order));
        let split = trap::traps(
            &open.inks,
            &open.cells,
            &self.orders,
            &open.intervening,
            open.composite_order,
        );

        if split {
            for (path, ink) in open.paths.iter().zip(&open.inks) {
                self.recorder.record(
                    core::slice::from_ref(path),
                    core::slice::from_ref(ink),
                    open.nested,
                    frame,
                    clips,
                    out,
                );
            }
        } else {
            self.recorder
                .record(&open.paths, &open.inks, open.nested, frame, clips, out);
            let composite = open.composite_order;
            self.raises.extend(
                open.paths
                    .iter()
                    .zip(&open.inks)
                    .filter(|(path, _)| path.order < composite)
                    .map(|(_, ink)| (*ink, composite)),
            );
        }
    }

    /// Sorts the passes by composite order, keeping their recorded sequence among equals, and
    /// moves their items so the passes' ranges follow each other.
    fn sort(&mut self, out: &mut VectorPlan) {
        let orders = &mut self.recorder.orders;
        if orders.is_sorted() {
            return;
        }

        self.sorted.clear();
        self.sorted.extend(
            orders
                .iter()
                .enumerate()
                .map(|(index, order)| (*order, index as u32)),
        );
        self.sorted.sort_unstable();

        self.passes.clear();
        self.items.clear();
        for (_, index) in &self.sorted {
            let mut pass = out.passes[*index as usize].clone();
            let start = self.items.len() as u32;
            self.items
                .extend_from_slice(&out.items[pass.items.start as usize..pass.items.end as usize]);
            pass.items = start..self.items.len() as u32;
            self.passes.push(pass);
        }

        core::mem::swap(&mut out.passes, &mut self.passes);
        core::mem::swap(&mut out.items, &mut self.items);
        orders.clear();
        orders.extend(self.sorted.iter().map(|(order, _)| *order));
    }
}
