//! Carried order node inks updated for the splices of their sequence, without summing the entries
//! the splices kept.

use iceblink::{DeviceRect, OrderEntry};
use smallvec::SmallVec;

use crate::derive::order::node::refresh_node;
use crate::scene::Scene;
use crate::scene::inks::{Carry, NodeInk};
use crate::scene::rows::Rows;
use crate::work::Work;

/// The most entries the splices of one node insert that are patched rather than summed.
const INSERTED: u32 = 8;

/// One splice of an order node's sequence: `removed` entries at position `at` replaced by
/// `inserted` others.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpliceOp {
    pub(crate) node: u32,
    pub(crate) at: u32,
    pub(crate) removed: u32,
    pub(crate) inserted: u32,
}

/// Reports whether the order node at `slot` can have its splices patched: it is carried and its
/// entries are ordered.
pub(super) fn patchable(scene: &Scene, slot: u32) -> bool {
    scene
        .orders
        .at(slot)
        .is_some_and(|record| record.ink.carry().is_some() && record.rows.is_some())
}

/// Updates the ink and rows of the order node at `slot` for the splices `Scene::splices` lists for
/// it, and returns the node's new ink, when it can.
///
/// It can when the node is carried and keeps its rows ordered, and the splices inserted a few
/// entries. Every inserted entry, and every kept node entry whose ink the refresh changes, must be
/// carried by the same scroll node to the same still pixels, or have no ink. The kept entries'
/// rows are renumbered, those entries' rows set, and the node's content is the union of its rows.
///
/// Returns `None`, with the node left stale, when it cannot or the scene lists no splice for the
/// node.
pub(super) fn patch(scene: &mut Scene, slot: u32, work: &mut Work) -> Option<NodeInk> {
    let start = scene.splices.partition_point(|op| op.node < slot);
    let end = start + scene.splices[start..].partition_point(|op| op.node == slot);
    let ops = &scene.splices[start..end];
    // Each inserted row is placed by a shift of the rows, so many insertions are summed instead.
    let inserted_total: u32 = ops.iter().map(|op| op.inserted).sum();
    if ops.is_empty() || inserted_total > INSERTED {
        return None;
    }
    let ops: SmallVec<[SpliceOp; 2]> = SmallVec::from_slice(ops);
    let record = scene.orders.at_mut(slot)?;
    let carry = record.ink.carry()?;
    // The rows are changed in place. A patch that fails leaves the node stale, and its sum then
    // replaces them.
    let mut rows = record.rows.take()?;

    // The positions of the inserted entries, followed through the later splices.
    let mut inserted: SmallVec<[usize; 4]> = SmallVec::new();
    for op in &ops {
        let (at, removed, count) = (op.at as usize, op.removed as usize, op.inserted as usize);
        rows.splice(at, removed, count);
        inserted.retain(|position| *position < at || *position >= at + removed);
        for position in &mut inserted {
            if *position >= at + removed {
                *position = *position - removed + count;
            }
        }
        inserted.extend(at..at + count);
    }
    inserted.sort_unstable();

    work.entries_summed += inserted.len() as u64;
    for &position in &inserted {
        let entry = scene.orders.at(slot)?.entries.get(position).copied()?;
        let Ok(ink) = content(scene, entry, carry, work) else {
            return put_back(scene, slot, rows);
        };
        rows.set(position, ink);
    }
    if !kept(scene, slot, carry, &inserted, &mut rows, work) || !rows.ordered() {
        return put_back(scene, slot, rows);
    }

    let ink = NodeInk::carried(rows.union(), carry);
    let refresh = scene.order_refreshes;
    let record = scene.orders.at_mut(slot)?;
    if record.ink != ink {
        record.changed_in = refresh;
    }
    record.ink = ink;
    record.rows = Some(rows);
    record.dirty = false;
    record.above = false;
    record.recarry = false;
    Some(ink)
}

/// Gives the order node at `slot` back the rows a failed patch took, and returns no ink.
fn put_back(scene: &mut Scene, slot: u32, rows: Rows) -> Option<NodeInk> {
    if let Some(record) = scene.orders.at_mut(slot) {
        record.rows = Some(rows);
    }
    None
}

/// Returns the content of entry `entry` that `carry` carries, refreshing a stale node entry
/// first, or `None` for an entry without an ink.
///
/// Fails for an entry with an ink that `carry` does not carry.
fn content(
    scene: &mut Scene,
    entry: OrderEntry,
    carry: Carry,
    work: &mut Work,
) -> Result<Option<DeviceRect>, ()> {
    let (held, content, device) = match entry {
        OrderEntry::Chunk(key) => {
            let Some(record) = scene.chunks.get(&key) else {
                return Ok(None);
            };
            let inks = &record.derived.inks;
            (
                inks.carry(),
                inks.content(),
                scene.device_inks(inks).union(),
            )
        }
        OrderEntry::Node(id) => {
            let ink = refresh_node(scene, id.slot(), work);
            (ink.carry(), ink.content(), scene.node_ink(&ink))
        }
    };
    match (held, content) {
        (Some(held), Some(content)) if held == carry => Ok(Some(content)),
        _ if device.is_none() => Ok(None),
        _ => Err(()),
    }
}

/// Updates the rows of the node entries of the order node at `slot` that the splices kept and
/// whose inks this refresh changes, and reports whether it could.
fn kept(
    scene: &mut Scene,
    slot: u32,
    carry: Carry,
    inserted: &[usize],
    rows: &mut Rows,
    work: &mut Work,
) -> bool {
    let mut position = 0;
    loop {
        let Some(entry) = scene
            .orders
            .at(slot)
            .and_then(|record| record.entries.get(position).copied())
        else {
            return true;
        };
        let at = position;
        position += 1;
        let OrderEntry::Node(id) = entry else {
            continue;
        };
        if inserted.binary_search(&at).is_ok() {
            continue;
        }
        let refresh = scene.order_refreshes;
        let changed = scene.orders.at(id.slot()).is_some_and(|child| {
            child.dirty || child.above || child.recarry || child.changed_in == refresh
        });
        if !changed {
            continue;
        }
        work.entries_summed += 1;
        match content(scene, entry, carry, work) {
            Ok(ink) => rows.set(at, ink),
            Err(()) => return false,
        }
    }
}
