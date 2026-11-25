//! The ink of every item of a chunk.

use iceblink::{DeviceRect, SpaceRef};

use crate::derive::carry::carried;
use crate::geometry::bounds::{DeviceBox, INK_LIMIT};
use crate::geometry::clip::{item_bounds, local_bounds};
use crate::geometry::ink::local_ink;
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::inks::ChunkInks;
use crate::work::Work;

/// Measures the ink of every item of `record` into `items`, in the content space of the scroll
/// node that carries it when one does, and counts the measurement in `work`.
pub(crate) fn measure(
    scene: &Scene,
    record: &ChunkRecord,
    work: &mut Work,
    mut items: Vec<Option<DeviceRect>>,
) -> ChunkInks {
    work.inks_computed += 1;

    if let Some(carry) = carried(scene, record, &mut items) {
        return ChunkInks::carried(items, carry);
    }
    let unions = chunk_inks(scene, record, &mut Vec::new(), &mut items);
    ChunkInks::on_device(items, unions.reach)
}

/// The unions of a chunk's items that [`chunk_inks`] returns.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Unions {
    /// The union of the item inks.
    pub(crate) ink: Option<DeviceRect>,
    /// The device box that holds every item before clips cut it, or `None` when no item has a
    /// box. It is everywhere when an edge of an item's box is not a number.
    pub(crate) reach: Option<DeviceBox>,
}

/// Computes the device ink of every item of `record` into `inks`, and returns their unions.
pub(crate) fn chunk_inks(
    scene: &Scene,
    record: &ChunkRecord,
    locals: &mut Vec<DeviceBox>,
    inks: &mut Vec<Option<DeviceRect>>,
) -> Unions {
    let chunk = &record.recorded;
    local_bounds(scene, chunk, record.offset, locals);
    inks.clear();

    let mut union: Option<DeviceRect> = None;
    let mut reach: Option<DeviceBox> = None;
    for &op in &chunk.ops {
        let ink = local_ink(chunk, op, &record.derived.runs).and_then(|local| {
            let slot = if local.space == SpaceRef::INHERIT {
                chunk.props.space.slot()
            } else {
                local.space.0
            };
            let world = scene.transforms.at(slot)?.world;
            let item = local.on_device(&world, record.offset);
            let edges = [item.x0, item.y0, item.x1, item.y1];
            // A box with an edge that is not a number holds no other box.
            let held = if edges.iter().any(|edge| edge.is_nan()) {
                DeviceBox::EVERYWHERE
            } else {
                item
            };
            reach = Some(reach.map_or(held, |reach| reach.union(held)));
            let clip = item_bounds(scene, chunk, locals, local.clip);
            item.intersect(clip).pixels(INK_LIMIT)
        });
        if let Some(ink) = ink {
            union = Some(union.map_or(ink, |held| held.union(ink)));
        }
        inks.push(ink);
    }
    Unions { ink: union, reach }
}
