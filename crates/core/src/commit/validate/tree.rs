//! Checking that parents form trees and that the order tree keeps its rules.

use iceblink::{OrderEntry, RejectReason};
use smallvec::SmallVec;

use crate::commit::journal::{Journal, OrderChange};
use crate::commit::precedence::Earliest;
use crate::output::Outputs;
use crate::scene::Scene;
use crate::small::SlotSet;

/// Notes in `earliest` every command that created or wrote a transform or clip that is its own
/// ancestor.
pub(super) fn check_cycles(scene: &Scene, journal: &Journal, earliest: &mut Earliest) {
    let transforms = scene.transforms.span();
    for change in &journal.transforms {
        let parent_of = |slot: u32| {
            scene
                .transforms
                .at(slot)
                .and_then(|record| record.node.parent)
                .map(|id| id.slot())
        };
        if loops(change.slot, transforms, parent_of) {
            earliest.note(change.command, RejectReason::Cycle);
        }
    }

    let clips = scene.clips.span();
    for change in &journal.clips {
        let parent_of = |slot: u32| {
            scene
                .clips
                .at(slot)
                .and_then(|record| record.node.parent)
                .map(|id| id.slot())
        };
        if loops(change.slot, clips, parent_of) {
            earliest.note(change.command, RejectReason::Cycle);
        }
    }
}

/// Notes in `earliest` every splice and root write that places an order node or chunk key in a
/// second place, and every splice that places an order node in its own subtree.
pub(super) fn check_order(
    scene: &Scene,
    outputs: &Outputs,
    journal: &Journal,
    entries: &[OrderEntry],
    earliest: &mut Earliest,
) {
    check_splices(scene, outputs, journal, entries, earliest);
    check_roots(scene, outputs, journal, earliest);
}

/// Notes in `earliest` every splice that places an order node or chunk key in a second place or
/// an order node in its own subtree.
fn check_splices(
    scene: &Scene,
    outputs: &Outputs,
    journal: &Journal,
    entries: &[OrderEntry],
    earliest: &mut Earliest,
) {
    let roots_of = |slot: u32| roots_of(scene, outputs, slot);
    let placed = |entry: &OrderEntry| match entry {
        OrderEntry::Chunk(key) => scene.placements.get(key).len(),
        OrderEntry::Node(id) => scene.node_placements.get(id.slot()).len() + roots_of(id.slot()),
    };
    let parent_of = |slot: u32| scene.node_placements.first(slot);

    // Only an entry a splice placed while it was placed already, or a root placed in a sequence,
    // can be held twice once the commit is applied.
    let mut doubled: SmallVec<[OrderEntry; 2]> = journal
        .doubled
        .iter()
        .filter(|entry| placed(entry) > 1)
        .copied()
        .collect();

    let mut acyclic = SlotSet::default();
    for change in &journal.orders {
        let OrderChange::Splice(splice) = change else {
            continue;
        };
        for entry in &entries[splice.entries.indices()] {
            let OrderEntry::Node(id) = entry else {
                continue;
            };
            if roots_of(id.slot()) > 0 && placed(entry) > 1 && !doubled.contains(entry) {
                doubled.push(*entry);
            }
            if in_cycle(id.slot(), scene.orders.span(), parent_of, &mut acyclic) {
                earliest.note(splice.command, RejectReason::Cycle);
            }
        }
    }

    if !doubled.is_empty() {
        for change in &journal.orders {
            if let OrderChange::Splice(splice) = change
                && entries[splice.entries.indices()]
                    .iter()
                    .any(|entry| doubled.contains(entry))
            {
                earliest.note(splice.command, RejectReason::OrderConflict);
            }
        }
    }
}

/// Notes in `earliest` every root write that places an order node in a second place.
fn check_roots(scene: &Scene, outputs: &Outputs, journal: &Journal, earliest: &mut Earliest) {
    for (index, write) in journal.roots.iter().enumerate() {
        // What a write placed is what the next write on its output replaced, or the root after
        // the commit.
        let later = journal.roots[index + 1..]
            .iter()
            .find(|later| later.output == write.output);
        let placed = match later {
            Some(later) => later.before,
            None => outputs.get(&write.output).and_then(|output| output.root),
        };
        let Some(root) = placed else {
            continue;
        };
        if root.0 != scene.client {
            continue;
        }
        let slot = root.1.slot();
        if scene.node_placements.get(slot).len() + roots_of(scene, outputs, slot) > 1 {
            earliest.note(write.command, RejectReason::OrderConflict);
        }
    }
}

/// Returns how many outputs have the order node at `slot` of `scene` as their root.
fn roots_of(scene: &Scene, outputs: &Outputs, slot: u32) -> usize {
    outputs
        .values()
        .filter(|output| {
            output
                .root
                .is_some_and(|(client, id)| client == scene.client && id.slot() == slot)
        })
        .count()
}

/// Reports whether walking up from `start` through `parent_of` comes back to `start`, as
/// [`loops`] does, and adds the nodes of a walk without a cycle to `acyclic`.
///
/// A walk stops at a node of `acyclic`, since the nodes above it hold no cycle.
fn in_cycle(
    start: u32,
    bound: usize,
    parent_of: impl Fn(u32) -> Option<u32>,
    acyclic: &mut SlotSet,
) -> bool {
    let mut path: SmallVec<[u32; 16]> = SmallVec::new();
    let mut current = start;

    for _ in 0..=bound {
        path.push(current);
        match parent_of(current) {
            Some(parent) if parent == start => return true,
            Some(parent) if !acyclic.contains(parent) => current = parent,
            Some(_) | None => {
                for slot in path {
                    acyclic.insert(slot);
                }
                return false;
            }
        }
    }
    // The walk ran into a cycle that `start` is not on, and a node on that cycle reports it.
    false
}

/// Reports whether walking up from `start` through `parent_of` comes back to `start`.
///
/// `bound` is more than the length of any chain without a cycle.
fn loops(start: u32, bound: usize, parent_of: impl Fn(u32) -> Option<u32>) -> bool {
    let mut current = start;
    for _ in 0..=bound {
        match parent_of(current) {
            Some(parent) if parent == start => return true,
            Some(parent) => current = parent,
            None => return false,
        }
    }
    // The walk ran into a cycle that `start` leads to but is not on, and a node on that cycle
    // reports it.
    false
}
