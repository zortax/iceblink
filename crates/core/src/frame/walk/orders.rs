//! The local orders of a lowering's instances, kept with the chunk across frames.

use std::sync::Arc;

use iceblink::{DeviceRect, Instance, LocalRect, Matrix4, SpaceRef};
use smallvec::SmallVec;

use crate::frame::lower::View;
use crate::geometry::bounds::{INK_LIMIT, map};
use crate::geometry::ink::local_ink;
use crate::plan::{BoundsTree, DrawOrder, local_orders};
use crate::scene::chunk::{ChunkRecord, Lane, LaneOrders, LocalOrders, Lowered, LoweredItem, Part};
use crate::tables::table::FrameTable;
use crate::work::Work;

/// Buffers computing local orders fills, kept between frames for their capacity.
#[derive(Debug, Default)]
pub(crate) struct OrderScratch {
    inks: Vec<DeviceRect>,
    members: Vec<u32>,
    locals: Vec<DrawOrder>,
    tree: BoundsTree,
}

/// Reports whether `orders` hold for the lowering of revision `revision` drawn at `offset`.
///
/// They hold when every space the instances are drawn in kept its linear part and moved by the
/// same whole-pixel translation as the others since the orders were computed.
fn hold(
    orders: &LocalOrders,
    revision: u64,
    offset: [f32; 2],
    spaces: &FrameTable<Matrix4>,
) -> bool {
    if orders.revision != revision {
        return false;
    }

    let mut shared: Option<[f32; 2]> = None;
    orders.spaces.iter().all(|&(slot, then)| {
        let Some(now) = spaces.get(slot) else {
            return false;
        };
        let by = if then == *now && orders.offset == offset {
            [0.0; 2]
        } else {
            let Some(by) = translation(&then, orders.offset, now, offset) else {
                return false;
            };
            by
        };
        // Rounded inks that are disjoint can meet after a fractional move.
        if by[0].fract() != 0.0 || by[1].fract() != 0.0 {
            return false;
        }
        match shared {
            Some(held) => (held[0] - by[0]).abs() < 1e-3 && (held[1] - by[1]).abs() < 1e-3,
            None => {
                shared = Some(by);
                true
            }
        }
    })
}

/// Returns the device translation from a point at `before_offset` under `before` to the point at
/// `after_offset` under `after`, when the two matrices share their linear part.
fn translation(
    before: &Matrix4,
    before_offset: [f32; 2],
    after: &Matrix4,
    after_offset: [f32; 2],
) -> Option<[f32; 2]> {
    let (a, b) = (before.to_affine2()?, after.to_affine2()?);
    if [a.a, a.b, a.c, a.d] != [b.a, b.b, b.c, b.d] {
        return None;
    }

    let [x, y] = [
        after_offset[0] - before_offset[0],
        after_offset[1] - before_offset[1],
    ];
    Some([
        a.a * x + a.c * y + (b.tx - a.tx),
        a.b * x + a.d * y + (b.ty - a.ty),
    ])
}

/// Computes the local orders of every instance of `lowered`, the lowering of `record`, in the
/// order the walk emits them.
///
/// An instance drawn with its item's ink is ordered by the item's ink before clipping, and a
/// sprite of an item with several instances by its own bounds.
#[inline(never)]
fn compute(
    record: &ChunkRecord,
    lowered: &Lowered,
    view: View<'_>,
    spaces: &FrameTable<Matrix4>,
    scratch: &mut OrderScratch,
) -> LocalOrders {
    let chunk = &lowered.chunk;
    let mut starts = [0u32; Lane::COUNT];
    let mut total = 0u32;
    for lane in Lane::ALL {
        starts[lane as usize] = total;
        total += lane.len(chunk) as u32;
    }

    let mut inks = Inks {
        record,
        lowered,
        view,
        spaces,
        starts,
        used: SmallVec::new(),
        scratch,
    };
    inks.scratch.inks.clear();
    inks.scratch.members.clear();
    for (position, item) in lowered.items.iter().enumerate() {
        match *item {
            LoweredItem::Instances { lane, start, len } => {
                let item_ink = inks.item(position);
                for index in start..start + len {
                    let ink = if len == 1 || !lane.sprite() {
                        item_ink
                    } else {
                        inks.sprite(lane, index)
                    };
                    inks.push(lane, index, ink);
                }
            }
            LoweredItem::Parts { start, len } => {
                let item_ink = inks.item(position);
                let range = start as usize..(start + len) as usize;
                for part in lowered.parts.get(range).unwrap_or(&[]) {
                    let Part::Instances { lane, start, len } = *part else {
                        continue;
                    };
                    for index in start..start + len {
                        let ink = if lane.sprite() {
                            inks.sprite(lane, index)
                        } else {
                            item_ink
                        };
                        inks.push(lane, index, ink);
                    }
                }
            }
            LoweredItem::Nothing | LoweredItem::Single => {}
        }
    }

    let used = core::mem::take(&mut inks.used);
    let scratch = inks.scratch;
    local_orders(&scratch.inks, &mut scratch.tree, &mut scratch.locals);

    let mut orders: Arc<[u32]> = core::iter::repeat_n(1, total as usize).collect();
    let slots = Arc::get_mut(&mut orders).expect("the orders were just made");
    for (&member, &order) in scratch.members.iter().zip(&scratch.locals) {
        slots[member as usize] = order;
    }

    LocalOrders {
        revision: lowered.revision,
        offset: record.offset,
        spaces: used,
        lanes: LaneOrders { starts, orders },
    }
}

/// The device inks of one lowering's instances, collected in emission order.
struct Inks<'a, 'b> {
    record: &'a ChunkRecord,
    lowered: &'a Lowered,
    view: View<'a>,
    spaces: &'a FrameTable<Matrix4>,
    starts: [u32; Lane::COUNT],
    /// The frame space slots the inks were measured in, with their world matrices.
    used: SmallVec<[(u32, Matrix4); 1]>,
    scratch: &'b mut OrderScratch,
}

impl Inks<'_, '_> {
    /// Returns the device ink of the item at `position` before clipping.
    fn item(&mut self, position: usize) -> Option<DeviceRect> {
        let chunk = &self.record.recorded;
        let op = *chunk.ops.get(position)?;
        let local = local_ink(chunk, op, &self.record.derived.runs)?;
        let slot = if local.space == SpaceRef::INHERIT {
            chunk.props.space.slot()
        } else {
            local.space.0
        };

        let space = self.view.transforms.at(slot)?;
        self.note(space.links.frame, space.world);
        local
            .on_device(&space.world, self.record.offset)
            .pixels(INK_LIMIT)
    }

    /// Returns the device ink of sprite `index` of `lane`.
    fn sprite(&mut self, lane: Lane, index: u32) -> Option<DeviceRect> {
        let chunk = &self.lowered.chunk;
        let base = self.lowered.base;
        let offset = self.record.offset;
        let delta = [offset[0] - base[0], offset[1] - base[1]];
        let at = index as usize;
        let (bounds, space) = match lane {
            Lane::Mono => chunk
                .mono
                .get(at)
                .map(|s| (s.moved(delta).bounds, s.space))?,
            Lane::Subpixel => chunk
                .subpixel
                .get(at)
                .map(|s| (s.moved(delta).bounds, s.space))?,
            Lane::Color => chunk
                .color
                .get(at)
                .map(|s| (s.moved(delta).bounds, s.space))?,
            _ => return None,
        };

        let world = *self.spaces.get(space)?;
        self.measured(space, world, bounds)
    }

    /// Returns the pixels `rect` touches under `world`, and notes the frame space it was measured
    /// in.
    fn measured(&mut self, slot: u32, world: Matrix4, rect: LocalRect) -> Option<DeviceRect> {
        self.note(slot, world);
        map(&world, rect).pixels(INK_LIMIT)
    }

    /// Notes that an ink was measured in frame space `slot` under `world`.
    fn note(&mut self, slot: u32, world: Matrix4) {
        if !self.used.iter().any(|&(held, _)| held == slot) {
            self.used.push((slot, world));
        }
    }

    /// Adds the ink of instance `index` of `lane`, when it has one.
    fn push(&mut self, lane: Lane, index: u32, ink: Option<DeviceRect>) {
        if let Some(ink) = ink {
            self.scratch.inks.push(ink);
            self.scratch
                .members
                .push(self.starts[lane as usize] + index);
        }
    }
}

/// Returns the local orders of the lowering of `record`, computing them when the lowering is new
/// or its spaces moved other than by one shared whole-pixel translation.
pub(super) fn current(
    view: View<'_>,
    record: &mut ChunkRecord,
    spaces: &FrameTable<Matrix4>,
    scratch: &mut OrderScratch,
    work: &mut Work,
) -> LaneOrders {
    let Some(lowered) = record.derived.lowered.as_ref() else {
        return LaneOrders::default();
    };
    if let Some(orders) = &record.derived.orders
        && hold(orders, lowered.revision, record.offset, spaces)
    {
        return orders.lanes.clone();
    }

    let orders = compute(record, lowered, view, spaces, scratch);
    work.local_orders += 1;
    let lanes = orders.lanes.clone();
    record.derived.orders = Some(orders);
    lanes
}
