//! Inks of the order nodes a commit changed, and the damage of splices and effect writes.

mod exact;
mod node;
mod patch;
mod recarry;
mod removed;
mod splice;
mod sum;

pub(crate) use self::exact::{content_ink, exact_ink, exact_ink_of};
pub(crate) use self::node::{group_region, refresh_chunk_parents};
pub(crate) use self::removed::removed_damage;
pub(crate) use self::splice::SpliceOp;
pub(crate) use self::sum::{NodeSum, Summed, isolates};

use iceblink::{ChunkKey, OrderEntry};
use smallvec::SmallVec;

use self::node::{Dirty, refresh_above};
use self::patch::patch;
use crate::commit::journal::{Journal, Op, OrderChange};
use crate::commit::net::nets;
use crate::derive::Derive;
use crate::derive::chunks::Touched;
use crate::derive::clips::ClipChanges;
use crate::derive::route::{Routes, damage};
use crate::geometry::coverage::CoverageChange;
use crate::scene::Scene;
use crate::small::{SlotSet, SmallMap, SmallSet};

/// The most replaced chunks one commit tracks without allocating.
const INLINE: usize = 8;

/// The most moved entries of one order node whose inks are patched rather than summed.
const PATCHED: usize = 4;

/// Returns the order nodes whose effect the commit wrote, through the effect or the node.
pub(super) fn redrawn_nodes(scene: &Scene, journal: &Journal) -> SmallVec<[u32; 8]> {
    let mut nodes: SmallVec<[u32; 8]> = SmallVec::new();
    for net in nets(&journal.effects, &scene.effects) {
        if let Some(record) = scene.effects.at(net.slot) {
            nodes.extend(record.orders.iter().copied());
        }
    }
    for change in &journal.order_nodes {
        if matches!(change.op, Op::Wrote(_)) {
            nodes.push(change.slot);
        }
    }
    nodes.sort_unstable();
    nodes.dedup();
    nodes
}

/// Damages the cached ink of every order node whose effect clip changed, cut to the clip's
/// coverage change, marks the node stale, and returns the nodes with their coverage changes.
pub(crate) fn clipped_groups(
    derive: &mut Derive<'_>,
    clips: &ClipChanges,
    routes: &Routes,
) -> Vec<(u32, CoverageChange)> {
    let mut nodes = Vec::new();
    for (&clip, coverage) in &clips.coverage {
        let Some(record) = derive.scene.clips.at(clip) else {
            continue;
        };
        for &effect in &record.effects {
            if let Some(effect) = derive.scene.effects.at(effect) {
                nodes.extend(effect.orders.iter().map(|&node| (node, coverage.clone())));
            }
        }
    }
    nodes.sort_unstable_by_key(|(node, _)| *node);
    nodes.dedup_by_key(|(node, _)| *node);
    for (node, coverage) in &nodes {
        let route = routes.of_group(derive.scene, *node);
        if let Some(ink) = exact_ink(derive.scene, *node) {
            for rect in coverage.cut(ink) {
                damage(derive.outputs, route, Some(rect));
            }
        }
        if let Some(record) = derive.scene.orders.at_mut(*node) {
            record.dirty = true;
        }
    }
    nodes
}

/// Recomputes the inks of the order nodes the commit made stale, and damages what splices
/// inserted, what effect writes redraw and what changed effect clips redraw.
pub(crate) fn refresh(
    derive: &mut Derive<'_>,
    journal: &Journal,
    touched: &Touched,
    routes: &Routes,
    entries: &[OrderEntry],
    clipped: &[(u32, CoverageChange)],
) {
    let scene = &mut *derive.scene;
    let mut spliced = Dirty::new();
    let mut dirty = Dirty::new();
    scene_dirty(scene, journal, &mut dirty, &mut spliced);
    // Nodes only splices changed. Stale nodes are summed in any case.
    let stale = SlotSet::of(&dirty);
    let mut only_spliced: Dirty = spliced
        .iter()
        .copied()
        .filter(|&node| !stale.contains(node))
        .collect();
    only_spliced.sort_unstable();
    only_spliced.dedup();

    let inserted = patched_inserts(scene, journal, &only_spliced, entries);
    let inserted = |key: ChunkKey, parent: u32| inserted.contains(&(parent, key));
    for &key in &touched.chunks {
        for &parent in scene.placements.get(&key) {
            if !inserted(key, parent) {
                dirty.push(parent);
            }
        }
    }
    dirty.extend_from_slice(&touched.nodes);
    dirty.extend(clipped.iter().map(|(node, _)| *node));
    dirty.sort_unstable();
    dirty.dedup();
    let sorted = dirty.len();
    let mut recarried = parents_of(scene, &touched.recarried);
    recarried.sort_unstable();
    recarried.dedup();

    // Patching finds the entry by a scan, so a node with many moved entries is summed instead.
    let mut parents: SmallMap<u32, SmallVec<[ChunkKey; PATCHED]>, PATCHED> = SmallMap::default();
    for &key in &touched.moved {
        match scene.placements.get(&key) {
            &[parent]
                if dirty.binary_search(&parent).is_err()
                    && only_spliced.binary_search(&parent).is_err()
                    && recarried.binary_search(&parent).is_err() =>
            {
                parents.get_or_insert_with(parent, SmallVec::new).push(key);
            }
            held => dirty.extend(
                held.iter()
                    .copied()
                    .filter(|&parent| !inserted(key, parent)),
            ),
        }
    }
    for (&parent, keys) in parents.iter() {
        if keys.len() > PATCHED {
            dirty.push(parent);
            continue;
        }
        for &key in keys {
            let stale = patch(scene, parent, key);
            dirty.extend_from_slice(&stale);
            // A parent summed again needs no patch for its other keys.
            if stale.contains(&parent) {
                break;
            }
        }
    }

    // A node only spliced is patched by its splices.
    let mut unlisted = difference(&only_spliced, &dirty[..sorted]);
    unlisted.retain(|node| !dirty[sorted..].contains(node));
    if !unlisted.is_empty() {
        queue_splices(scene, journal, &unlisted);
    }
    dirty.extend_from_slice(&unlisted);
    refresh_above(scene, dirty, &recarried, derive.work);
    scene.splices.clear();

    damage_inserted(derive, journal, routes, entries);
    damage_redrawn(derive, journal, routes, clipped);
}

/// Returns the chunks, with their order node, that splices inserted into the nodes in
/// `only_spliced` whose splices can be patched.
///
/// The splice's patch takes such a chunk in, so its parent is not summed again for it.
fn patched_inserts(
    scene: &Scene,
    journal: &Journal,
    only_spliced: &[u32],
    entries: &[OrderEntry],
) -> SmallSet<(u32, ChunkKey), INLINE> {
    let mut inserted = SmallSet::default();
    for change in &journal.orders {
        if let OrderChange::Splice(splice) = change
            && only_spliced.binary_search(&splice.node).is_ok()
            && splice::patchable(scene, splice.node)
        {
            for entry in &entries[splice.entries.indices()] {
                if let OrderEntry::Chunk(key) = *entry {
                    inserted.add((splice.node, key));
                }
            }
        }
    }
    inserted
}

/// Lists the splices of the nodes in the sorted list `nodes` that can be patched in
/// `Scene::splices`, sorted by node.
fn queue_splices(scene: &mut Scene, journal: &Journal, nodes: &[u32]) {
    for change in &journal.orders {
        if let OrderChange::Splice(splice) = change
            && nodes.binary_search(&splice.node).is_ok()
            && splice::patchable(scene, splice.node)
        {
            scene.splices.push(SpliceOp {
                node: splice.node,
                at: splice.at,
                removed: splice.removed.len,
                inserted: splice.inserted,
            });
        }
    }
    // A stable sort keeps each node's splices in the order they applied.
    scene.splices.sort_by_key(|op| op.node);
}

/// Damages the ink of every entry the commit's splices inserted.
fn damage_inserted(
    derive: &mut Derive<'_>,
    journal: &Journal,
    routes: &Routes,
    entries: &[OrderEntry],
) {
    for change in &journal.orders {
        let OrderChange::Splice(splice) = change else {
            continue;
        };
        let inserted = &entries[splice.entries.indices()];
        if inserted.is_empty() {
            continue;
        }

        let mut route = None;
        for entry in inserted {
            let scene = &*derive.scene;
            let ink = match entry {
                OrderEntry::Chunk(key) => scene
                    .chunks
                    .get(key)
                    .and_then(|r| scene.device_inks(&r.derived.inks).union()),
                OrderEntry::Node(id) => exact_ink(scene, id.slot()),
            };
            if ink.is_some() {
                let route = *route.get_or_insert_with(|| routes.of_node(scene, splice.node));
                damage(derive.outputs, route, ink);
            }
        }
    }
}

/// Damages the ink of every order node whose effect the commit wrote, and of every other node in
/// `clipped` where its effect clip covers differently.
fn damage_redrawn(
    derive: &mut Derive<'_>,
    journal: &Journal,
    routes: &Routes,
    clipped: &[(u32, CoverageChange)],
) {
    let redrawn = redrawn_nodes(derive.scene, journal);
    for &slot in &redrawn {
        let ink = exact_ink(derive.scene, slot);
        let route = routes.of_group(derive.scene, slot);
        damage(derive.outputs, route, ink);
    }

    for (slot, coverage) in clipped {
        if redrawn.contains(slot) {
            continue;
        }
        let route = routes.of_group(derive.scene, *slot);
        if let Some(ink) = exact_ink(derive.scene, *slot) {
            for rect in coverage.cut(ink) {
                damage(derive.outputs, route, Some(rect));
            }
        }
    }
}

/// Returns the nodes of the sorted list `nodes` that the sorted list `without` leaves out.
fn difference(nodes: &[u32], without: &[u32]) -> Dirty {
    let mut left = without.iter().peekable();
    nodes
        .iter()
        .copied()
        .filter(|&node| {
            while left.next_if(|&&held| held < node).is_some() {}
            left.peek() != Some(&&node)
        })
        .collect()
}

/// Returns the order nodes whose sequences hold any of `keys`.
pub(super) fn parents_of(scene: &Scene, keys: &[ChunkKey]) -> Dirty {
    keys.iter()
        .flat_map(|key| scene.placements.get(key))
        .copied()
        .collect()
}

/// Collects the order nodes already marked stale by the commit into `dirty`, and the nodes it
/// spliced into `spliced`, leaving out the nodes it removed.
fn scene_dirty(scene: &Scene, journal: &Journal, dirty: &mut Dirty, spliced: &mut Dirty) {
    let mut gone = SlotSet::default();
    for change in &journal.order_nodes {
        match change.op {
            Op::Removed(_) => gone.insert(change.slot),
            Op::Created(_) => gone.remove(change.slot),
            Op::Wrote(_) => {}
        }
    }
    for change in &journal.orders {
        if let OrderChange::Splice(splice) = change
            && !gone.contains(splice.node)
        {
            spliced.push(splice.node);
        }
    }
    dirty.extend(
        journal
            .order_nodes
            .iter()
            .map(|change| change.slot)
            .filter(|&slot| !gone.contains(slot)),
    );
    for net in nets(&journal.effects, &scene.effects) {
        if let Some(record) = scene.effects.at(net.slot) {
            dirty.extend(record.orders.iter().copied());
        }
    }
}

/// Damages the whole of every output whose root the commit set.
pub(crate) fn roots(derive: &mut Derive<'_>, journal: &Journal) {
    for write in &journal.roots {
        if let Some(output) = derive.outputs.get_mut(&write.output) {
            output.damage_all();
        }
    }
}
