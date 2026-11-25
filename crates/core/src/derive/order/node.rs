//! The inks of order nodes, summed from their entries.

use iceblink::{ChunkKey, DeviceRect, OrderEntry, read_extent};
use smallvec::SmallVec;

use crate::derive::links::unwatch;
use crate::derive::order::exact::exact_ink;
use crate::derive::order::recarry::recarried;
use crate::derive::order::sum::{NodeSum, Summed, isolates};
use crate::derive::order::{parents_of, splice};
use crate::geometry::bounds::{DeviceBox, INK_LIMIT, of_pixels};
use crate::scene::Scene;
use crate::scene::inks::NodeInk;
use crate::scene::rows::Rows;
use crate::work::Work;

/// The most emptied rows the scene keeps for later sums.
const SPARE_ROWS: usize = 16;

/// The most order nodes one commit makes stale that are tracked without allocating.
const INLINE: usize = 16;

/// Order nodes whose inks are stale.
pub(super) type Dirty = SmallVec<[u32; INLINE]>;

/// Recomputes the inks of the order nodes `dirty` and `recarried_nodes`, and of the nodes above
/// them whose inks can depend on a changed ink.
///
/// The chunk entries of the nodes `recarried_nodes` changed only the still pixels they are cut
/// to. Such a node takes the carry its entries share, as [`recarried`] describes, or is summed
/// when it cannot. A node in both lists is summed.
///
/// A node above is summed again when the ink of one of its node entries changed, or when its
/// effect isolates. A node above whose changed node entries changed only their carry takes their
/// carry instead. The others keep their inks.
pub(super) fn refresh_above(
    scene: &mut Scene,
    mut dirty: Dirty,
    recarried_nodes: &[u32],
    work: &mut Work,
) {
    // Zero is the refresh no node changed in or was visited by.
    scene.order_refreshes = scene.order_refreshes.wrapping_add(1).max(1);
    let refresh = scene.order_refreshes;
    let listed = dirty.len();
    dirty.extend_from_slice(recarried_nodes);
    let recarried_end = dirty.len();

    let mut index = 0;
    while index < dirty.len() {
        let slot = dirty[index];
        index += 1;
        let Some(record) = scene.orders.at_mut(slot) else {
            continue;
        };
        if record.visited_in == refresh {
            continue;
        }
        record.visited_in = refresh;
        if index <= listed {
            record.dirty = true;
        } else if index <= recarried_end {
            record.recarry = true;
        } else {
            record.above = true;
        }
        dirty.extend_from_slice(scene.node_placements.get(slot));
    }

    for &slot in &dirty {
        refresh_node(scene, slot, work);
    }
}

/// Recomputes the inks of the order nodes that hold any of `keys`, and of the nodes above them,
/// as [`refresh_above`] does.
pub(crate) fn refresh_chunk_parents(scene: &mut Scene, keys: &[ChunkKey], work: &mut Work) {
    let dirty = parents_of(scene, keys);
    refresh_above(scene, dirty, &[], work);
}

/// Recomputes the inks of an order node whose cache is stale, and of its stale descendants first,
/// counts the entries summed, and returns the node's ink.
#[inline]
pub(super) fn refresh_node(scene: &mut Scene, slot: u32, work: &mut Work) -> NodeInk {
    let Some(record) = scene.orders.at(slot) else {
        return NodeInk::default();
    };
    if record.dirty {
        sum_node(scene, slot, work)
    } else if record.recarry {
        recarried(scene, slot, work).unwrap_or_else(|| sum_node(scene, slot, work))
    } else if record.above {
        settle(scene, slot, work)
    } else {
        record.ink
    }
}

/// Recomputes the stale descendants of an order node above a stale node, and returns the node's
/// ink.
///
/// When the ink of one of its entries changed, the node takes the carry its entries share, as
/// [`recarried`] describes, or is summed again.
#[inline(never)]
fn settle(scene: &mut Scene, slot: u32, work: &mut Work) -> NodeInk {
    // A group's written region is summed from its entries' exact inks, which can change while
    // their cached inks hold.
    if isolates(scene, slot) {
        return sum_node(scene, slot, work);
    }
    match entries_changed(scene, slot, work) {
        EntriesChanged::None => {}
        EntriesChanged::Carry => {
            return recarried(scene, slot, work).unwrap_or_else(|| sum_node(scene, slot, work));
        }
        EntriesChanged::Ink => return sum_node(scene, slot, work),
    }
    match scene.orders.at_mut(slot) {
        Some(record) => {
            record.above = false;
            record.ink
        }
        None => NodeInk::default(),
    }
}

/// Sums the inks of an order node's entries, refreshing its stale descendants first, and returns
/// the node's new ink.
///
/// The node's watchers are updated: the scroll nodes that carry one of its entries without
/// carrying the node watch it.
#[inline(never)]
fn sum_node(scene: &mut Scene, slot: u32, work: &mut Work) -> NodeInk {
    if !scene.splices.is_empty()
        && let Some(ink) = splice::patch(scene, slot, work)
    {
        return ink;
    }
    let len = scene
        .orders
        .at(slot)
        .map_or(0, |record| record.entries.len());
    work.entries_summed += len as u64;

    let isolating = isolates(scene, slot);
    let mut sum = NodeSum::with_rows(spare_rows(scene), spare_rows(scene));
    for index in 0..len {
        // Each entry is read again, since refreshing a child borrows the scene.
        let Some(entry) = scene
            .orders
            .at(slot)
            .and_then(|r| r.entries.get(index).copied())
        else {
            break;
        };
        match entry {
            OrderEntry::Chunk(key) => match scene.chunks.get(&key) {
                Some(record) => sum.add_chunk(scene, &record.derived.inks),
                None => sum.skip(),
            },
            OrderEntry::Node(id) => {
                let ink = refresh_node(scene, id.slot(), work);
                let device = if isolating {
                    exact_ink(scene, id.slot())
                } else {
                    scene.node_ink(&ink)
                };
                sum.add_node(device, ink);
            }
        }
    }

    let (
        Summed {
            ink,
            watching,
            rows,
        },
        unused,
    ) = sum.finish_with_spare(scene, slot);
    for rows in unused {
        keep_rows(scene, Some(rows));
    }
    if reads_around(scene, slot) {
        scene.reading_groups.insert(slot);
    } else if !scene.reading_groups.is_empty() {
        scene.reading_groups.remove(&slot);
    }

    let refresh = scene.order_refreshes;
    let Some(record) = scene.orders.at_mut(slot) else {
        return ink;
    };
    let watched = core::mem::replace(&mut record.watching, watching.clone());
    if record.ink != ink {
        record.changed_in = refresh;
        if only_carry(record.ink, ink) {
            record.recarried_in = refresh;
        }
    }
    record.ink = ink;
    let replaced = core::mem::replace(&mut record.rows, rows);
    record.dirty = false;
    record.above = false;
    record.recarry = false;
    keep_rows(scene, replaced);

    unwatch(scene, slot, &watched);
    for scroll in watching {
        if let Some(record) = scene.scrolls.at_mut(scroll) {
            record.watchers.insert(slot);
        }
    }
    ink
}

/// How the inks of an order node's node entries changed in a refresh.
enum EntriesChanged {
    /// No changed ink.
    None,
    /// A change of the first changed ink's carry alone.
    Carry,
    /// Any other change of the first changed ink.
    Ink,
}

/// Recomputes the stale node entries of the order node at `slot` up to the first whose ink the
/// refresh changed, and reports how that ink changed.
fn entries_changed(scene: &mut Scene, slot: u32, work: &mut Work) -> EntriesChanged {
    let refresh = scene.order_refreshes;
    let mut index = 0;
    loop {
        let entry = scene
            .orders
            .at(slot)
            .and_then(|record| record.entries.get(index).copied());
        index += 1;
        let child = match entry {
            None => return EntriesChanged::None,
            Some(OrderEntry::Chunk(_)) => continue,
            Some(OrderEntry::Node(id)) => id.slot(),
        };
        let Some(record) = scene.orders.at(child) else {
            continue;
        };
        if record.dirty || record.above || record.recarry {
            refresh_node(scene, child, work);
        }
        match scene.orders.at(child) {
            Some(record) if record.changed_in == refresh && record.recarried_in == refresh => {
                return EntriesChanged::Carry;
            }
            Some(record) if record.changed_in == refresh => return EntriesChanged::Ink,
            _ => {}
        }
    }
}

/// Reports whether `new` differs from `old` only in the still pixels a carry cuts the content to.
fn only_carry(old: NodeInk, new: NodeInk) -> bool {
    match (old.carry(), new.carry()) {
        (Some(old_carry), Some(new_carry)) => {
            old_carry.scroll == new_carry.scroll && old.content() == new.content()
        }
        _ => false,
    }
}

/// Returns empty rows, reusing the capacity of rows the scene keeps.
fn spare_rows(scene: &mut Scene) -> Rows {
    // Rows grow as entries with an ink arrive, so a node without ink allocates none.
    scene.spare_rows.pop().unwrap_or_default()
}

/// Keeps `rows` emptied for a later sum, unless the scene keeps enough.
fn keep_rows(scene: &mut Scene, rows: Option<Rows>) {
    if let Some(mut rows) = rows
        && scene.spare_rows.len() < SPARE_ROWS
    {
        rows.clear();
        scene.spare_rows.push(rows);
    }
}

/// Returns the written region of an order node's group, or its content ink when its effect does
/// not isolate.
pub(crate) fn group_region(
    scene: &Scene,
    slot: u32,
    content: Option<DeviceRect>,
) -> Option<DeviceRect> {
    let content = content?;
    let Some(effect) = scene
        .orders
        .at(slot)
        .and_then(|record| record.node.effect)
        .and_then(|effect| scene.effects.at(effect.slot()))
    else {
        return Some(content);
    };
    if !effect.node.isolates() {
        return Some(content);
    }

    let extent = read_extent(of_pixels(content).rect(), &effect.node.filters);
    let clip = effect
        .node
        .clip
        .and_then(|clip| scene.clips.at(clip.slot()))
        .map_or(DeviceBox::EVERYWHERE, |clip| clip.bounds);
    DeviceBox::of(extent).intersect(clip).pixels(INK_LIMIT)
}

/// Reports whether an order node's group has a filter that reads outside what it writes.
fn reads_around(scene: &Scene, slot: u32) -> bool {
    scene
        .orders
        .at(slot)
        .and_then(|record| record.node.effect)
        .and_then(|effect| scene.effects.at(effect.slot()))
        .is_some_and(|effect| {
            effect.node.isolates()
                && read_extent([0.0, 0.0, 1.0, 1.0], &effect.node.filters) != [0.0, 0.0, 1.0, 1.0]
        })
}
