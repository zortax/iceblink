//! Refreshing one chunk the commit changed or moved.

use iceblink::{ChunkKey, DeviceRect, OrderEntry};

use crate::commit::journal::Journal;
use crate::derive::Derive;
use crate::derive::chunks::KeyChanges;
use crate::derive::chunks::clipped::clip_change;
use crate::derive::chunks::flags::{reads_around, reads_time};
use crate::derive::chunks::inline::hold_inline;
use crate::derive::chunks::locals::{allocate_locals, rewrite_locals};
use crate::derive::chunks::loose::set_inks;
use crate::derive::chunks::original::{drop_derived, put_back, take_original};
use crate::derive::chunks::record::{Moved, Replaced, rerecorded};
use crate::derive::clips::ClipChanges;
use crate::derive::diff::{same_frame, same_item};
use crate::derive::inks::measure;
use crate::derive::links::{Cause, refers_to};
use crate::derive::route::{Route, RouteCache, Routes, damage};
use crate::derive::runs::{Measure, reuse};
use crate::derive::scroll::{EntryMove, Moves, Travels, accumulate};
use crate::derive::world::WorldChanges;
use crate::frame::lower::retire;
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::inks::{Carry, ChunkInks};
use crate::scene::kind::ObjectRef;

/// Why one chunk is refreshed.
#[derive(Clone, Copy)]
pub(super) struct Change {
    pub(super) key: ChunkKey,
    /// Every cause merged, a recording counting as a write.
    pub(super) cause: Cause,
    /// The cause of a move of a space or clip the chunk is drawn through, if any.
    pub(super) motion: Option<Cause>,
}

/// What refreshing the chunks of one commit shares.
pub(super) struct Pass<'a> {
    pub(super) world: &'a WorldChanges,
    pub(super) clips: &'a ClipChanges,
    pub(super) routes: &'a Routes,
    pub(super) travels: &'a Travels,
    /// The objects the commit removed and created again at the same slot.
    pub(super) replaced: &'a [ObjectRef],
    pub(super) moves: &'a mut Moves,
    pub(super) found: RouteCache,
    pub(super) inks: ItemInks,
}

/// The item inks of the chunk being refreshed, before and after the commit, kept between
/// commits for their capacity.
#[derive(Debug, Default)]
pub(crate) struct ItemInks {
    new: Vec<Option<DeviceRect>>,
    old: Vec<Option<DeviceRect>>,
    /// An emptied list that the next chunk's new inks fill.
    spare: Vec<Option<DeviceRect>>,
}

/// How a chunk's refresh changed what the order nodes holding it sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Summary {
    /// The union and the carry are as before.
    Same,
    /// The union changed, and the chunk was on the device before and after.
    Moved,
    /// The carry changed, or the chunk is new or gone.
    Changed,
}

/// Recomputes the inks of one chunk, damages what changed and keeps or drops its lowering.
///
/// Returns how the chunk's union or carry changed, which the order nodes holding it sum.
pub(super) fn refresh_one(
    derive: &mut Derive<'_>,
    journal: &mut Journal,
    changes: Option<&KeyChanges>,
    change: Change,
    pass: &mut Pass<'_>,
) -> Summary {
    let Change { key, motion, .. } = change;
    let recorded = changes.is_some_and(|changes| changes.recorded);
    // The new recording takes its colours before the old one drops them, so they keep their slots.
    if recorded {
        hold_inline(derive, key);
    }
    let original = recorded.then(|| take_original(journal, changes)).flatten();
    let mut summed = original
        .as_ref()
        .map(|original| summed_of(&original.derived.inks));
    if let Some(original) = &original {
        reuse(derive.scene, key, original);
    }

    let mut measure_runs = Measure {
        scene: derive.scene,
        tables: derive.tables,
        content: derive.content,
        outputs: derive.outputs,
        text: derive.text,
    };
    crate::derive::runs::refresh(&mut measure_runs, key);

    // The flags follow the recording, and the clock flag also the shaders it draws with.
    let flags = (recorded || !journal.shaders.is_empty()).then(|| {
        (
            reads_time(derive.scene, key),
            reads_around(derive.scene, key),
        )
    });
    let Some(record) = derive.scene.chunks.get_mut(&key) else {
        if let Some(original) = original {
            put_back(journal, changes, original);
        }
        return Summary::Changed;
    };
    if let Some((reads_time, reads_around)) = flags {
        record.derived.reads_time = reads_time;
        record.derived.reads_around = reads_around;
    }

    // A new record's inks are empty, so their list can take the new inks.
    let items = recorded
        .then(|| derive.scene.chunks.get_mut(&key))
        .flatten()
        .map(|record| record.derived.inks.take_items())
        .unwrap_or_else(|| core::mem::take(&mut pass.inks.spare));
    // An identical recording that nothing moved draws the inks it had.
    let kept = original
        .as_ref()
        .filter(|original| motion.is_none() && identical(original, &derive.scene.chunks[&key]));
    let inks = match kept {
        Some(original) => original.derived.inks.copy_into(items),
        None => measure(derive.scene, &derive.scene.chunks[&key], derive.work, items),
    };
    let ItemInks { new, old, .. } = &mut pass.inks;
    new.clear();
    new.extend(derive.scene.device_inks(&inks).items());
    old.clear();
    let route = pass.found.of_chunk(pass.routes, derive.scene, key);

    if recorded {
        rerecord(derive, journal, changes, change, pass, original, route);
    } else {
        let Some(held) = remeasure(derive, changes, change, pass, route) else {
            return Summary::Changed;
        };
        summed = Some(held);
    }

    let now = summed_of(&inks);
    let change = match summed {
        Some(old) if old == now => Summary::Same,
        Some((_, None)) if now.1.is_none() => Summary::Moved,
        _ => Summary::Changed,
    };
    set_inks(derive.scene, key, inks);
    if let Some((reads_time, reads_around)) = flags {
        set_member(&mut derive.scene.timed, key, reads_time);
        set_member(&mut derive.scene.reading, key, reads_around);
    }
    change
}

/// Damages a re-recorded chunk against `original`, the record it replaced if there is one, and
/// moves the original's lowering and local clip slots to the chunk when both record the same
/// items.
///
/// The chunk's new device inks are in `pass`. Its old device inks are added there too.
fn rerecord(
    derive: &mut Derive<'_>,
    journal: &mut Journal,
    changes: Option<&KeyChanges>,
    change: Change,
    pass: &mut Pass<'_>,
    mut original: Option<ChunkRecord>,
    route: Option<Route>,
) {
    let Change { key, motion, .. } = change;
    let ItemInks { new, old, .. } = &mut pass.inks;
    if let Some(original) = &original {
        before(
            derive.scene,
            pass.travels,
            pass.clips,
            &original.derived.inks,
            old,
        );
    }
    if let (Some(Cause::Scroll(scroll)), Some(route), Some(_)) = (motion, route, original.as_ref())
    {
        let moving = EntryMove {
            scroll,
            route,
            entry: OrderEntry::Chunk(key),
            old,
            new,
        };
        accumulate(derive, pass.moves, pass.world, moving);
    }

    // Clip writes alone move the chunk's pixels only where the clips cover differently.
    let clipped = (motion == Some(Cause::Write))
        .then(|| clip_change(derive.scene, key, None, pass.world, pass.clips))
        .filter(|change| !change.is_whole());
    let moved = match motion {
        Some(Cause::Write) if clipped.is_some() => Moved::Clipped,
        Some(Cause::Write) => Moved::Redraw,
        Some(Cause::Scroll(_)) => Moved::Scrolled,
        None => Moved::Still,
    };
    let fresh = derive
        .scene
        .chunks
        .get(&key)
        .is_some_and(|record| refers_to(&record.derived.deps, pass.replaced));
    let replaced = Replaced {
        key,
        original: original.as_ref(),
        old,
        inks: new,
        route,
        moved,
        clipped: clipped.as_ref(),
        fresh,
    };
    let same = rerecorded(derive, replaced);

    match original.as_mut() {
        Some(original) if same => {
            let carried = &mut original.derived;
            derive.tables.release_inline(&carried.inline, derive.serial);
            carried.inline.clear();
            if let Some(record) = derive.scene.chunks.get_mut(&key) {
                record.derived.lowered = carried.lowered.take();
                record.derived.orders = carried.orders.take();
                core::mem::swap(&mut record.derived.local_frames, &mut carried.local_frames);
                carried.local_frames.clear();
            }
            let offset = derive.scene.chunks.get(&key).map(|record| record.offset);
            if motion.is_some() || offset != Some(original.offset) {
                rewrite_locals(derive, key);
            }
        }
        Some(original) => {
            drop_derived(derive, &mut original.derived);
            allocate_locals(derive, key);
        }
        None => allocate_locals(derive, key),
    }
    if let Some(original) = original {
        put_back(journal, changes, original);
    }
}

/// Damages a chunk the commit moved or changed without recording it again, and returns what the
/// order nodes holding it summed of its inks before, or `None` when the chunk is gone.
///
/// The chunk's new device inks are in `pass`. Its old device inks are added there too.
fn remeasure(
    derive: &mut Derive<'_>,
    changes: Option<&KeyChanges>,
    change: Change,
    pass: &mut Pass<'_>,
    route: Option<Route>,
) -> Option<(Option<DeviceRect>, Option<Carry>)> {
    let Change { key, cause, .. } = change;
    let ItemInks { new, old, spare } = &mut pass.inks;
    let record = derive.scene.chunks.get_mut(&key)?;
    let mut held = record.derived.inks.take();
    let summed = summed_of(&held);
    before(derive.scene, pass.travels, pass.clips, &held, old);
    *spare = held.take_items();

    match (cause, route) {
        (Cause::Scroll(scroll), Some(route)) => {
            let moving = EntryMove {
                scroll,
                route,
                entry: OrderEntry::Chunk(key),
                old,
                new,
            };
            accumulate(derive, pass.moves, pass.world, moving);
        }
        _ => {
            // An item can change pixels inside an unchanged ink, such as by a fractional
            // move, so every item is damaged where it was and where it is, cut to where its
            // clips cover differently.
            let change = clip_change(derive.scene, key, changes, pass.world, pass.clips);
            for ink in old.iter().chain(new.iter()).flatten() {
                for rect in change.cut(*ink) {
                    damage(derive.outputs, route, Some(rect));
                }
            }
        }
    }

    let props = changes.is_some_and(|changes| changes.props);
    let stale = props.then(|| derive.scene.chunks.get_mut(&key)?.derived.lowered.take());
    if let Some(lowered) = stale.flatten() {
        retire(lowered, derive.revisions, derive.content, derive.tables);
    }
    rewrite_locals(derive, key);
    Some(summed)
}

/// What the order nodes holding a chunk sum of its inks: their union and their carry.
fn summed_of(inks: &ChunkInks) -> (Option<DeviceRect>, Option<Carry>) {
    (inks.content(), inks.carry())
}

/// Adds the device inks a chunk had before the commit, from its stored inks, to `old`.
fn before(
    scene: &Scene,
    travels: &Travels,
    clips: &ClipChanges,
    inks: &ChunkInks,
    old: &mut Vec<Option<DeviceRect>>,
) {
    let travel = inks
        .carry()
        .map_or([0; 2], |carry| travels.before(scene, carry.scroll));
    old.extend(clips.device_inks_before(scene, inks, travel).items());
}

fn set_member(set: &mut rustc_hash::FxHashSet<ChunkKey>, key: ChunkKey, member: bool) {
    if member {
        set.insert(key);
    } else {
        set.remove(&key);
    }
}

/// Reports whether `new` records the same items as `original` at the same offset.
fn identical(original: &ChunkRecord, new: &ChunkRecord) -> bool {
    let (old, recorded) = (&original.recorded, &new.recorded);
    original.offset == new.offset
        && old.ops.len() == recorded.ops.len()
        && same_frame(old, recorded)
        && (0..recorded.ops.len()).all(|position| same_item(old, recorded, position))
}
