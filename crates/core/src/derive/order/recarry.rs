//! Order node inks whose entries changed only the still pixels they are cut to.

use iceblink::OrderEntry;

use crate::derive::order::node::refresh_node;
use crate::scene::Scene;
use crate::scene::inks::{Carry, NodeInk};
use crate::work::Work;

/// Sets the carry of the order node at `slot` to the carry its entries share, keeping its content
/// and rows, and returns its new ink, or returns `None` when the node must be summed.
///
/// The node's stale node entries are refreshed first. The node's content and rows stay valid when
/// its chunk entries changed at most their carry, and when these conditions are met:
///
/// 1. The node is carried and has no effect.
/// 2. At least one entry has an ink.
/// 3. Every entry with an ink is carried, all of them with the same carry, by the scroll node that
///    carried the node.
/// 4. Every node entry whose ink this refresh changed changed only its carry.
#[inline(never)]
pub(super) fn recarried(scene: &mut Scene, slot: u32, work: &mut Work) -> Option<NodeInk> {
    let record = scene.orders.at(slot)?;
    if record.node.effect.is_some() {
        return None;
    }
    let old = record.ink;
    let scroll = old.carry()?.scroll;
    let refresh = scene.order_refreshes;

    let mut shared: Option<Carry> = None;
    let mut index = 0;
    while let Some(entry) = scene
        .orders
        .at(slot)
        .and_then(|record| record.entries.get(index).copied())
    {
        index += 1;
        let (carry, content) = match entry {
            OrderEntry::Chunk(key) => match scene.chunks.get(&key) {
                Some(record) => (record.derived.inks.carry(), record.derived.inks.content()),
                None => continue,
            },
            OrderEntry::Node(id) => {
                let ink = refresh_node(scene, id.slot(), work);
                let child = scene.orders.at(id.slot())?;
                if child.changed_in == refresh && child.recarried_in != refresh {
                    return None;
                }
                (ink.carry(), ink.content())
            }
        };
        if content.is_none() {
            continue;
        }
        let carry = carry?;
        match shared {
            Some(held) if held != carry => return None,
            _ => shared = Some(carry),
        }
    }
    let carry = shared.filter(|carry| carry.scroll == scroll)?;

    let ink = NodeInk::carried(old.content(), carry);
    let record = scene.orders.at_mut(slot)?;
    if record.ink != ink {
        record.changed_in = refresh;
        record.recarried_in = refresh;
        work.carries_replaced += 1;
    }
    record.ink = ink;
    record.dirty = false;
    record.above = false;
    record.recarry = false;
    Some(ink)
}
