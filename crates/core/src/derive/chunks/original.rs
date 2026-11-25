//! The records chunks held before a commit, and dropping what was derived from them.

use iceblink::ChunkKey;

use crate::commit::journal::{ChunkChange, Journal};
use crate::derive::Derive;
use crate::derive::chunks::{Changes, KeyChanges, Keys};
use crate::derive::route::{Routes, damage};
use crate::frame::lower::retire;
use crate::scene::chunk::{ChunkDerived, ChunkRecord};

/// Takes the record a key held before the commit out of the journal, when a replacement holds it.
pub(super) fn take_original(
    journal: &mut Journal,
    changes: Option<&KeyChanges>,
) -> Option<ChunkRecord> {
    let position = changes?.first_replaced?;
    match journal.chunks.get_mut(position)? {
        ChunkChange::Replaced { before, .. } => {
            let index = (*before)? as usize;
            journal.replaced.get_mut(index)?.take()
        }
        ChunkChange::Offset { .. } | ChunkChange::Props { .. } => None,
    }
}

/// Puts a record taken with `take_original` back, so its buffers are recycled.
pub(super) fn put_back(journal: &mut Journal, changes: Option<&KeyChanges>, record: ChunkRecord) {
    let Some(position) = changes.and_then(|changes| changes.first_replaced) else {
        return;
    };
    if let Some(ChunkChange::Replaced {
        before: Some(index),
        ..
    }) = journal.chunks.get(position)
        && let Some(slot) = journal.replaced.get_mut(*index as usize)
    {
        *slot = Some(record);
    }
}

/// Damages the old ink of a removed chunk, and drops what was derived from it.
pub(super) fn removed(
    derive: &mut Derive<'_>,
    journal: &mut Journal,
    keys: &Keys,
    key: ChunkKey,
    routes: &Routes,
    moved: &Changes<'_>,
) {
    let changes = keys.get(&key);
    let Some(mut original) = take_original(journal, changes) else {
        return;
    };
    let route = routes.of_chunk(derive.scene, key);
    let inks = &original.derived.inks;
    let travel = inks.carry().map_or([0; 2], |carry| {
        moved.travels.before(derive.scene, carry.scroll)
    });
    let before = moved.clips.device_inks_before(derive.scene, inks, travel);
    for ink in before.items() {
        damage(derive.outputs, route, ink);
    }
    drop_derived(derive, &mut original.derived);
    derive.scene.timed.remove(&key);
    derive.scene.reading.remove(&key);
    put_back(journal, changes, original);
}

/// Releases the frame slots of a chunk's local clips and inline colours, and retires its
/// lowering.
pub(super) fn drop_derived(derive: &mut Derive<'_>, derived: &mut ChunkDerived) {
    for slot in derived.local_frames.drain(..) {
        derive.tables.clips.release(slot, derive.serial);
    }
    let colors = core::mem::take(&mut derived.inline);
    derive.tables.release_inline(&colors, derive.serial);
    if let Some(lowered) = derived.lowered.take() {
        retire(lowered, derive.revisions, derive.content, derive.tables);
    }
}
