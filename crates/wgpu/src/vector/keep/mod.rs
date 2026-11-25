//! A rasteriser that keeps the rasters of keyed passes across frames.

mod copy;
mod store;

use iceblink::geom::Point;
use iceblink::{DeviceRect, MemoryReport, PassKey, PlannedPass, VectorBackend, VectorPlan};
use rustc_hash::FxHashSet;

use crate::vector::frame::VectorFrame;
use crate::vector::raster::VectorRaster;
use crate::vector::resource::{ResourcedPass, ResourcedPlan, VectorError, VectorTarget};

use self::store::Store;

/// The bit that marks a target as a kept raster.
const KEPT: u64 = 1 << 62;

/// A rasteriser that keeps the raster of a keyed pass drawn in two frames in a row, and
/// composites a kept raster for a later pass with the same key and region size instead of
/// drawing that pass again.
///
/// The inner rasteriser draws all other passes. It must place passes in targets below 2⁶², and
/// it keeps nothing unless it reports the texture of its targets through
/// [`VectorRaster::texture`]. Kept rasters stay until they hold more than a byte cap, and then
/// those composited longest ago are dropped first.
pub struct Keeping {
    inner: Box<dyn VectorRaster>,
    store: Store,
    /// Where each pass of the frame's plan is drawn from.
    slots: Vec<Slot>,
    /// The passes the inner rasteriser draws.
    drawn: VectorPlan,
    /// The inner rasteriser's resources for `drawn`.
    inner_plan: ResourcedPlan,
    /// The keys and region sizes of the keyed passes drawn in the frame before and in this one.
    seen: [FxHashSet<(PassKey, [i32; 2])>; 2],
    cap: u64,
    hits: u64,
    misses: u64,
}

/// Where one pass of a frame is drawn from.
#[derive(Clone, Copy, Debug)]
enum Slot {
    /// Pass `index` of the inner plan, with the pass's key and region size.
    Drawn {
        index: usize,
        key: PassKey,
        size: [i32; 2],
    },
    /// The kept raster `index`.
    Kept(u32),
}

impl Keeping {
    /// Wraps `inner`, keeping at most 64 MiB of rasters.
    pub fn new(inner: Box<dyn VectorRaster>) -> Self {
        Self {
            inner,
            store: Store::default(),
            slots: Vec::new(),
            drawn: VectorPlan::default(),
            inner_plan: ResourcedPlan::default(),
            seen: Default::default(),
            cap: Store::BYTE_CAP,
            hits: 0,
            misses: 0,
        }
    }

    /// Routes `pass` of `plan` to its kept raster, or to the inner rasteriser when none is kept.
    fn route(&mut self, plan: &VectorPlan, pass: &PlannedPass) {
        let size = [pass.region.size.width, pass.region.size.height];
        let kept = (pass.key != PassKey::NONE)
            .then(|| self.store.find(pass.key, size))
            .flatten();
        if let Some(index) = kept {
            self.hits += 1;
            self.slots.push(Slot::Kept(index));
            return;
        }

        if pass.key != PassKey::NONE {
            self.seen[1].insert((pass.key, size));
        }
        let first = self.drawn.items.len() as u32;
        self.drawn.items.extend_from_slice(plan.items_of(pass));
        self.drawn.passes.push(PlannedPass {
            items: first..self.drawn.items.len() as u32,
            ..pass.clone()
        });
        self.slots.push(Slot::Drawn {
            index: self.drawn.passes.len() - 1,
            key: pass.key,
            size,
        });
    }

    /// Returns the resourced plan of `plan`, with each pass placed in its kept raster or where
    /// the inner rasteriser placed it.
    fn resourced(&self, plan: &VectorPlan) -> ResourcedPlan {
        let mut resourced = ResourcedPlan::resourcing(plan);
        // A kept pass takes the round of the pass before it, so rounds never decrease.
        let mut round = 0;
        for (pass, slot) in plan.passes.iter().zip(&self.slots) {
            let resourced_pass = match *slot {
                Slot::Drawn { index, .. } => self.inner_plan.passes.get(index).map(|drawn| {
                    debug_assert!(drawn.target == VectorTarget::NONE || drawn.target.0 < KEPT);
                    round = drawn.round;
                    ResourcedPass {
                        items: pass.items.clone(),
                        ..drawn.clone()
                    }
                }),
                Slot::Kept(index) => Some(placed(
                    pass,
                    VectorTarget(KEPT | u64::from(index)),
                    DeviceRect::new(Point::new(0, 0), pass.region.size),
                    round,
                )),
            };
            resourced.passes.push(
                resourced_pass
                    .unwrap_or_else(|| placed(pass, VectorTarget::NONE, pass.region, round)),
            );
        }
        resourced
    }

    /// Returns how many passes precede pass `prepared` of the inner plan in the frame's plan.
    fn frame_index(&self, prepared: usize) -> usize {
        self.slots
            .iter()
            .position(|slot| matches!(*slot, Slot::Drawn { index, .. } if index >= prepared))
            .unwrap_or(self.slots.len())
    }
}

impl VectorRaster for Keeping {
    fn backend(&self) -> VectorBackend {
        self.inner.backend()
    }

    fn flat_gradients(&self) -> bool {
        self.inner.flat_gradients()
    }

    fn plan(&mut self, plan: &VectorPlan) -> ResourcedPlan {
        self.store.begin();
        let [before, now] = &mut self.seen;
        core::mem::swap(before, now);
        now.clear();
        self.slots.clear();
        self.drawn.clear();
        if plan.is_empty() {
            self.inner_plan = ResourcedPlan::default();
            return ResourcedPlan::default();
        }

        for pass in &plan.passes {
            self.route(plan, pass);
        }
        self.inner_plan = if self.drawn.is_empty() {
            ResourcedPlan::default()
        } else {
            self.inner.plan(&self.drawn)
        };

        self.resourced(plan)
    }

    fn clear_targets(&mut self, _plan: &ResourcedPlan, round: usize) {
        if !self.inner_plan.round(round).is_empty() {
            self.inner.clear_targets(&self.inner_plan, round);
        }
    }

    fn prepare(&mut self, frame: &mut VectorFrame<'_>) -> Result<(), VectorError> {
        let round = frame.round;
        let result = if self.inner_plan.round(round).is_empty() {
            Ok(())
        } else {
            let mut inner = VectorFrame {
                plan: &self.inner_plan,
                paths: frame.paths,
                clips: frame.clips,
                paints: frame.paints,
                spaces: frame.spaces,
                round,
                device: frame.device,
                queue: frame.queue,
            };
            self.inner.prepare(&mut inner)
        };

        let prepared = match &result {
            Ok(()) => usize::MAX,
            Err(VectorError::OutOfCapacity { prepared, .. }) => *prepared,
            Err(_) => return result,
        };
        self.misses += copy::keep(self, frame, prepared);

        result.map_err(|failure| match failure {
            VectorError::OutOfCapacity { detail, prepared } => VectorError::OutOfCapacity {
                detail,
                prepared: self.frame_index(prepared),
            },
            other => other,
        })
    }

    fn view(&self, target: VectorTarget) -> Option<&wgpu::TextureView> {
        match kept(target) {
            Some(index) => self.store.get(index).map(|raster| &raster.view),
            None => self.inner.view(target),
        }
    }

    fn texture(&self, target: VectorTarget) -> Option<(&wgpu::Texture, u32)> {
        match kept(target) {
            Some(index) => self.store.get(index).map(|raster| (&raster.texture, 0)),
            None => self.inner.texture(target),
        }
    }

    fn memory(&self) -> MemoryReport {
        let mut report = self.inner.memory();
        report.vector += self.store.bytes();
        report
    }

    fn release_idle_resources(&mut self) -> u64 {
        self.inner.release_idle_resources() + self.store.release()
    }

    fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        self.inner.extra_stats(out);
        out("keep.hits", self.hits);
        out("keep.misses", self.misses);
        out("keep.rasters", self.store.len() as u64);
        out("keep.bytes", self.store.bytes());
    }
}

/// Returns `pass` resourced at `raster_region` of `target` in `round`.
fn placed(
    pass: &PlannedPass,
    target: VectorTarget,
    raster_region: DeviceRect,
    round: usize,
) -> ResourcedPass {
    ResourcedPass {
        region: pass.region,
        raster_region,
        target,
        round,
        items: pass.items.clone(),
        clip: pass.clip,
        instanced: pass.instanced,
        clamped: pass.clamped || pass.overhangs,
    }
}

/// Returns the index of the kept raster `target` refers to, or `None` for a target of the inner
/// rasteriser.
fn kept(target: VectorTarget) -> Option<u32> {
    (target != VectorTarget::NONE && target.0 & KEPT != 0).then_some((target.0 & !KEPT) as u32)
}
