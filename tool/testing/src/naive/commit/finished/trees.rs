//! The tree rules of transform parents, clip parents and the order tree.

use std::collections::{HashMap, HashSet};

use iceblink::{ChunkKey, OrderEntry, RejectReason};

use crate::naive::commit::draft::Draft;
use crate::naive::commit::precedence::Earliest;
use crate::naive::scene::Table;

/// Notes every broken `OrderConflict` and `Cycle` rule at the command it is attributed to.
pub(super) fn check(draft: &Draft<'_>, earliest: &mut Earliest) {
    conflicts(draft, earliest);
    cycles(draft, earliest);
}

/// Notes `OrderConflict` at every command that placed an order node or chunk key that appears in
/// two places.
fn conflicts(draft: &Draft<'_>, earliest: &mut Earliest) {
    let placing = &draft.placing;
    let twice = placed_twice(draft);
    for (index, entry) in &placing.entries {
        let doubled = match entry {
            OrderEntry::Chunk(key) => twice.keys.contains(key),
            OrderEntry::Node(node) => twice.nodes.contains(&node.slot()),
        };
        if doubled {
            earliest.note(*index, RejectReason::OrderConflict);
        }
    }

    for (index, root) in &placing.roots {
        if twice.nodes.contains(&root.slot()) {
            earliest.note(*index, RejectReason::OrderConflict);
        }
    }
}

/// Notes `Cycle` at the earliest command that shaped each parent chain that comes back to where
/// it started.
fn cycles(draft: &Draft<'_>, earliest: &mut Earliest) {
    let client = &draft.client;
    let placing = &draft.placing;
    let orders = order_parents(draft);
    for slot in on_cycles(&orders) {
        let inserted = placing
            .entries
            .iter()
            .find(|(_, entry)| matches!(entry, OrderEntry::Node(node) if node.slot() == slot));
        if let Some((index, _)) = inserted {
            earliest.note(*index, RejectReason::Cycle);
        }
    }

    let transforms = parent_slots(&client.transforms, |node| node.parent.map(|id| id.slot()));
    for slot in on_cycles(&transforms) {
        if let Some(index) = placing.transforms.get(&slot) {
            earliest.note(*index, RejectReason::Cycle);
        }
    }

    let clips = parent_slots(&client.clips, |node| node.parent.map(|id| id.slot()));
    for slot in on_cycles(&clips) {
        if let Some(index) = placing.clips.get(&slot) {
            earliest.note(*index, RejectReason::Cycle);
        }
    }
}

/// The order nodes and chunk keys that appear more than once among the client's entries and
/// output roots.
#[derive(Default)]
struct Twice {
    nodes: HashSet<u32>,
    keys: HashSet<ChunkKey>,
}

fn placed_twice(draft: &Draft<'_>) -> Twice {
    let mut twice = Twice::default();
    let mut nodes = HashSet::new();
    let mut keys = HashSet::new();

    let roots = draft
        .roots
        .values()
        .filter(|(owner, _)| *owner == draft.client_id)
        .map(|(_, root)| root.slot());
    for root in roots {
        if !nodes.insert(root) {
            twice.nodes.insert(root);
        }
    }

    for (_, _, order) in draft.client.orders.iter() {
        for entry in &order.entries {
            match entry {
                OrderEntry::Chunk(key) => {
                    if !keys.insert(*key) {
                        twice.keys.insert(*key);
                    }
                }
                OrderEntry::Node(child) => {
                    if !nodes.insert(child.slot()) {
                        twice.nodes.insert(child.slot());
                    }
                }
            }
        }
    }

    twice
}

/// Returns the parent of each order node: a node whose entries hold it.
fn order_parents(draft: &Draft<'_>) -> HashMap<u32, u32> {
    let mut parents = HashMap::new();
    for (slot, _, order) in draft.client.orders.iter() {
        for entry in &order.entries {
            if let OrderEntry::Node(child) = entry {
                parents.insert(child.slot(), slot);
            }
        }
    }

    parents
}

fn parent_slots<T>(table: &Table<T>, parent: impl Fn(&T) -> Option<u32>) -> HashMap<u32, u32> {
    table
        .iter()
        .filter_map(|(slot, _, node)| Some((slot, parent(node)?)))
        .collect()
}

/// Returns every slot from which following `parents` comes back to the slot itself.
fn on_cycles(parents: &HashMap<u32, u32>) -> Vec<u32> {
    let mut cycles = Vec::new();
    let mut finished = HashSet::new();
    for start in parents.keys() {
        let mut path = Vec::new();
        let mut at = *start;
        while !finished.contains(&at) {
            if let Some(position) = path.iter().position(|slot| *slot == at) {
                cycles.extend_from_slice(&path[position..]);
                break;
            }

            path.push(at);
            match parents.get(&at) {
                Some(parent) => at = *parent,
                None => break,
            }
        }

        finished.extend(path);
    }

    cycles
}
