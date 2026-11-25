//! Order node inks updated for one entry, without summing the nodes' entries again.

use iceblink::{ChunkKey, DeviceRect, OrderEntry, OrderId};
use smallvec::SmallVec;

use crate::derive::order::sum::isolates;
use crate::scene::Scene;
use crate::scene::inks::NodeInk;

/// Updates the inks of order node `slot` and the nodes above it for its entry chunk `key`, whose
/// device ink changed while the chunk stayed on the device, and returns the nodes left stale.
///
/// The nodes are updated from `slot` upwards:
///
/// - If a node isolates, is stale, is carried or cannot keep its rows ordered, it is left stale.
/// - If a node's ink holds the entry's new ink, it keeps its ink, and the nodes above it are
///   marked wide.
/// - Otherwise the node's ink grows by the entry's new ink, and its parent is updated next.
///
/// A patched ink may be wider than its entries' union, and is marked so. The inks updated before
/// a node is left stale still bound their subtrees.
///
/// The stale nodes need their entries summed again:
///
/// - the node that could not be updated,
/// - the parents of a node with several parents,
/// - the first isolating node above, whose written region follows its entries' exact inks.
pub(super) fn patch(scene: &mut Scene, slot: u32, key: ChunkKey) -> Stale {
    let failed = |slot: u32| Stale::from_slice(&[slot]);
    let Some(chunk) = scene.chunks.get(&key) else {
        return failed(slot);
    };
    let Some(ink) = scene.device_inks(&chunk.derived.inks).union() else {
        return failed(slot);
    };
    let mut entry = OrderEntry::Chunk(key);
    let mut slot = slot;
    let mut ink = ink;
    // A tree is never deeper than the order nodes it holds.
    for _ in 0..=scene.orders.span() {
        match patch_one(scene, slot, entry, ink) {
            Step::Held => return widen_above(scene, slot),
            Step::Failed => return failed(slot),
            Step::Grew(grown, id) => match scene.node_placements.get(slot) {
                [] => return Stale::new(),
                &[parent] => {
                    entry = OrderEntry::Node(id);
                    slot = parent;
                    ink = grown;
                }
                parents => return Stale::from_slice(parents),
            },
        }
    }
    failed(slot)
}

/// Marks the inks of the nodes above order node `slot` wide, up to the first node that isolates
/// or has several parents, and returns the nodes it stopped at.
fn widen_above(scene: &mut Scene, slot: u32) -> Stale {
    let mut slot = slot;
    // A tree is never deeper than the order nodes it holds.
    for _ in 0..=scene.orders.span() {
        let parent = match scene.node_placements.get(slot) {
            [] => return Stale::new(),
            &[parent] => parent,
            parents => return Stale::from_slice(parents),
        };
        if isolates(scene, parent) {
            return Stale::from_slice(&[parent]);
        }
        let Some(record) = scene.orders.at_mut(parent) else {
            return Stale::new();
        };
        record.ink = record.ink.widened();
        slot = parent;
    }
    Stale::new()
}

/// The order nodes a patch leaves stale.
pub(super) type Stale = SmallVec<[u32; 2]>;

/// What updating one node for one entry did.
enum Step {
    /// The node's ink holds the entry's ink, so nothing above changes.
    Held,
    /// The node's ink grew to the given ink, and the node above must hold it.
    Grew(DeviceRect, OrderId),
    /// The node cannot be updated in place.
    Failed,
}

fn patch_one(scene: &mut Scene, slot: u32, entry: OrderEntry, ink: DeviceRect) -> Step {
    if isolates(scene, slot) {
        return Step::Failed;
    }
    let Some(record) = scene.orders.at_mut(slot) else {
        return Step::Failed;
    };
    let held = record.ink;
    if record.dirty || held.carry().is_some() {
        return Step::Failed;
    }
    if let Some(rows) = &mut record.rows {
        let Some(position) = record.entries.iter().position(|held| *held == entry) else {
            return Step::Failed;
        };
        if !rows.update(position, ink) {
            return Step::Failed;
        }
    }
    match held.content() {
        Some(node) if node.contains_rect(ink) => {
            record.ink = NodeInk::on_device(Some(node), true);
            Step::Held
        }
        content => {
            let grown = content.map_or(ink, |node| node.union(ink));
            record.ink = NodeInk::on_device(Some(grown), true);
            Step::Grew(grown, record.id)
        }
    }
}
