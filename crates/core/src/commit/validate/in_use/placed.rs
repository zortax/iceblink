//! Checking that no order node or output root holds a removed order node.

use iceblink::{OrderEntry, OrderId, RejectReason};

use crate::commit::journal::Journal;
use crate::commit::precedence::Earliest;
use crate::output::Outputs;
use crate::scene::Scene;

/// Notes every removal of an order node that an order node or an output root still holds.
///
/// An entry or root that names an order node the commit created at the slot holds only that node.
pub(super) fn placed_nodes(
    scene: &Scene,
    outputs: &Outputs,
    journal: &Journal,
    earliest: &mut Earliest,
) {
    for (slot, command) in journal.removed_order_slots() {
        let live = scene.orders.at(slot).map(|record| record.id);
        let removed = |id: OrderId| id.slot() == slot && Some(id) != live;
        let placed = match live {
            None => scene.node_placements.contains(slot),
            Some(_) => scene.node_placements.get(slot).iter().any(|&parent| {
                scene.orders.at(parent).is_some_and(|record| {
                    record
                        .entries
                        .iter()
                        .any(|entry| matches!(*entry, OrderEntry::Node(id) if removed(id)))
                })
            }),
        };
        let rooted = outputs.values().any(|output| {
            output
                .root
                .is_some_and(|(client, id)| client == scene.client && removed(id))
        });
        if placed || rooted {
            earliest.note(command, RejectReason::InUse);
        }
    }
}
