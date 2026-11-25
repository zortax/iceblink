//! The ink of an order subtree as the union of its items' inks.

use iceblink::{DeviceRect, OrderEntry};

use crate::scene::Scene;
use crate::scene::inks::moved;
use crate::scene::record::OrderRecord;

/// Returns the ink of the subtree of order node `slot`: the union of its items' device inks, or
/// the written region when the node's effect isolates.
pub(crate) fn exact_ink(scene: &Scene, slot: u32) -> Option<DeviceRect> {
    exact(scene, scene.orders.at(slot)?, 0)
}

/// Returns the content ink of order node `slot`: the union of its entries' inks, with each
/// isolating child's written region in place of its content.
pub(crate) fn content_ink(scene: &Scene, slot: u32) -> Option<DeviceRect> {
    entries(scene, scene.orders.at(slot)?, 0)
}

/// Returns the ink of the subtree of `record`, as `exact_ink` does.
pub(crate) fn exact_ink_of(scene: &Scene, record: &OrderRecord) -> Option<DeviceRect> {
    exact(scene, record, 0)
}

fn exact(scene: &Scene, record: &OrderRecord, depth: usize) -> Option<DeviceRect> {
    let ink = &record.ink;
    if !ink.wide() || depth > scene.orders.span() {
        return scene.node_ink(ink);
    }
    // Content inside the still pixels, or wholly outside them, is not cut by them.
    if let Some(carry) = ink.carry() {
        let content = moved(ink.content()?, scene.travel(carry.scroll));
        let still = scene.still_pixels(carry.still);
        if still.contains_rect(content) {
            return Some(content);
        }
        if !still.intersects(content) {
            return None;
        }
    }

    entries(scene, record, depth)
}

fn entries(scene: &Scene, record: &OrderRecord, depth: usize) -> Option<DeviceRect> {
    let mut union: Option<DeviceRect> = None;
    for entry in &record.entries {
        let ink = match *entry {
            OrderEntry::Chunk(key) => scene
                .chunks
                .get(&key)
                .and_then(|chunk| scene.device_inks(&chunk.derived.inks).union()),
            OrderEntry::Node(id) => scene
                .orders
                .at(id.slot())
                .and_then(|child| exact(scene, child, depth + 1)),
        };
        if let Some(ink) = ink {
            union = Some(union.map_or(ink, |held| held.union(ink)));
        }
    }
    union
}
