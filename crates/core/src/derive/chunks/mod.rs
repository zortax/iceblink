//! Inks, local clip entries and lowerings of the chunks a commit changed, and their damage.

mod clipped;
mod flags;
mod inline;
mod kept;
mod locals;
mod loose;
mod marks;
mod motions;
mod one;
mod original;
mod paths;
mod reaches;
mod record;
mod reshaped;
mod restill;
mod still_moved;
mod users;
mod watchers;

pub(crate) use self::loose::{forget_loose, set_inks};
pub(crate) use self::paths::forget_removed_paths;
pub(crate) use self::users::damage_users;

use self::locals::rewrite_locals;
pub(crate) use self::motions::WalkedLists;
use self::motions::{Quiet, find_motions};
pub(crate) use self::one::ItemInks;
use self::one::{Change, Pass, Summary, refresh_one};
use self::original::removed;
use self::reaches::keep_reaches;
use self::restill::{LastChange, Restilled, restill};
use self::still_moved::still_moved;
use self::users::slots;
use self::watchers::moved_by;

use iceblink::ChunkKey;
use smallvec::SmallVec;

use crate::commit::journal::{ChunkChange, Journal};
use crate::commit::validate::ChunkNet;
use crate::derive::Derive;
use crate::derive::clips::ClipChanges;
use crate::derive::links::Cause;
use crate::derive::route::Routes;
use crate::derive::scroll::{Moves, Travels};
use crate::derive::world::WorldChanges;
use crate::scene::Scene;
use crate::scene::chunk::RunLowering;
use crate::scene::kind::ObjectRef;
use crate::small::SmallMap;

/// What a commit changed that can move or rewrite chunks.
pub(crate) struct Changes<'a> {
    pub(crate) nets: &'a [ChunkNet],
    pub(crate) world: &'a WorldChanges,
    pub(crate) clips: &'a ClipChanges,
    pub(crate) travels: &'a Travels,
    /// The objects the commit removed and created again at the same slot.
    pub(crate) replaced: &'a [ObjectRef],
}

/// The chunks and order nodes whose inks the commit may have changed, which the order nodes
/// holding them sum again.
#[derive(Debug, Default)]
pub(crate) struct Touched {
    /// Chunks that are new or gone, or whose union or carry changed.
    pub(crate) chunks: SmallVec<[ChunkKey; INLINE]>,
    /// Chunks on the device before and after the commit whose union alone changed.
    pub(crate) moved: SmallVec<[ChunkKey; INLINE]>,
    /// Carried chunks whose still pixels alone changed.
    pub(crate) recarried: SmallVec<[ChunkKey; INLINE]>,
    /// Order nodes whose entries' device inks moved with a travel or with still pixels.
    pub(crate) nodes: SmallVec<[u32; INLINE]>,
}

/// The most chunk keys a commit changes or moves whose bookkeeping is kept inline.
const INLINE: usize = 16;

/// The journal's changes of each changed chunk key.
pub(super) type Keys = SmallMap<ChunkKey, KeyChanges, INLINE>;

/// The cause of each chunk's move.
pub(super) type Causes = SmallMap<ChunkKey, Cause, INLINE>;

/// The journal's changes of one changed chunk key.
#[derive(Debug, Default)]
pub(super) struct KeyChanges {
    /// The position of the first replacement, whose `before` is the record from before the commit.
    pub(super) first_replaced: Option<usize>,
    /// Whether the key was recorded or removed.
    recorded: bool,
    /// Whether the chunk's props were written.
    props: bool,
}

/// Recomputes every chunk the commit changed or moved, sends the damage of each change to the
/// output that draws it, and adds what the order nodes must sum again to `touched`.
///
/// The chunks a scroll node carries are not recomputed when the commit moved them by its travel.
/// A chunk the commit did not write keeps its inks when the clips that changed draw it the same.
pub(crate) fn refresh(
    derive: &mut Derive<'_>,
    journal: &mut Journal,
    changes: &Changes<'_>,
    routes: &Routes,
    moves: &mut Moves,
    touched: &mut Touched,
) {
    let mut keys = Keys::default();
    key_changes(journal, &mut keys);
    let mut motions = Causes::default();
    let mut last = LastChange::default();
    let lists = derive.scene.walked.take();
    let mut walked = find_motions(derive, changes, &keys, &mut motions, &mut last, lists);
    derive.work.clip_users_checked += walked.checked;
    *derive.mismatches += walked.mismatches;
    rewrite_kept(derive, &mut walked.kept, &motions);

    let mut causes = motions.clone();
    for net in changes.nets.iter().filter(|net| net.after.is_some()) {
        merge(&mut causes, net.key, Cause::Write);
    }

    for net in changes.nets.iter().filter(|net| net.after.is_none()) {
        removed(derive, journal, &keys, net.key, routes, changes);
        touched.chunks.push(net.key);
    }

    let mut pass = Pass {
        world: changes.world,
        clips: changes.clips,
        routes,
        travels: changes.travels,
        replaced: changes.replaced,
        moves,
        found: core::mem::take(&mut derive.scene.routes),
        inks: core::mem::take(&mut derive.scene.item_inks),
    };
    pass.found.keep_for(routes, journal);

    let mut stilled = quiet_scrolls(&walked.quiet, &motions);
    let stills = &mut walked.stills;
    stills.sort_unstable();
    stills.dedup();
    for &key in stills.iter() {
        if causes.contains_key(&key) {
            continue;
        }
        match restill(derive, key, changes, &mut pass, &mut last) {
            Some(Restilled::Recarried) => touched.recarried.push(key),
            Some(Restilled::Moved(scroll)) => {
                if !stilled.contains(&scroll) {
                    stilled.push(scroll);
                }
            }
            Some(Restilled::Same) => {}
            None => merge(&mut causes, key, Cause::Write),
        }
    }
    stilled.sort_unstable();
    still_moved(derive.scene, &stilled, touched);

    refresh_caused(
        derive, journal, &keys, &motions, &causes, &mut pass, touched,
    );

    keep_reaches(derive.scene, &walked.unkept);
    derive.scene.walked.keep(walked);
    derive.scene.routes = core::mem::take(&mut pass.found);
    derive.scene.item_inks = core::mem::take(&mut pass.inks);
    slots(derive, journal, routes);
    touch_watchers(derive.scene, changes.travels, touched);
}

/// Rewrites the local clips of the chunks in `kept` that are not in `motions`.
fn rewrite_kept(derive: &mut Derive<'_>, kept: &mut Vec<ChunkKey>, motions: &Causes) {
    // A chunk a clip change leaves alone needs only its local clips, whose chains hold the
    // changed clips.
    kept.sort_unstable();
    kept.dedup();
    for &key in kept.iter() {
        if !motions.contains_key(&key) {
            rewrite_locals(derive, key);
        }
    }
}

/// Returns the scroll nodes whose content moved with the still pixels of the chunks restilled
/// quietly, leaving out the chunks the motions took after they were restilled.
fn quiet_scrolls(quiet: &[Quiet], motions: &Causes) -> SmallVec<[u32; 2]> {
    let mut stilled: SmallVec<[u32; 2]> = SmallVec::new();
    // Every net's key is a key of the journal, which no chunk restilled quietly is.
    for quiet in quiet {
        let taken = quiet.motions != motions.len() && motions.contains_key(&quiet.key);
        if let (false, Restilled::Moved(scroll)) = (taken, quiet.outcome)
            && !stilled.contains(&scroll)
        {
            stilled.push(scroll);
        }
    }
    stilled
}

/// Refreshes every chunk in `causes` in key order, recounts the bytes of those whose lowerings
/// changed, and adds what the order nodes must sum again to `touched`.
fn refresh_caused(
    derive: &mut Derive<'_>,
    journal: &mut Journal,
    keys: &Keys,
    motions: &Causes,
    causes: &Causes,
    pass: &mut Pass<'_>,
    touched: &mut Touched,
) {
    let mut ordered: SmallVec<[(ChunkKey, Cause); INLINE]> =
        causes.iter().map(|(&key, &cause)| (key, cause)).collect();
    ordered.sort_unstable_by_key(|(key, _)| *key);

    for (key, cause) in ordered {
        let change = Change {
            key,
            cause,
            motion: motions.get(&key).copied(),
        };
        let before = lowered_parts(derive.scene, key);
        let summary = refresh_one(derive, journal, keys.get(&key), change, pass);

        // Only a new recording, or a lowering made or dropped, changes the chunk's bytes.
        let recorded = keys.get(&key).is_some_and(|changes| changes.recorded);
        if recorded || lowered_parts(derive.scene, key) != before {
            derive.scene.recount(key);
            derive.work.bytes_counted += 1;
        }

        match summary {
            Summary::Same => {}
            Summary::Moved => touched.moved.push(key),
            Summary::Changed => touched.chunks.push(key),
        }
    }
}

/// Adds to `touched` the watchers of every scroll node in `travels` whose entries the travel moved.
fn touch_watchers(scene: &Scene, travels: &Travels, touched: &mut Touched) {
    for scroll in travels.scrolls() {
        if let Some(record) = scene.scrolls.at(scroll) {
            touched.nodes.extend(
                record
                    .watchers
                    .iter()
                    .copied()
                    .filter(|&watcher| moved_by(scene, travels, watcher, scroll)),
            );
        }
    }
}

/// Adds `cause` to the causes of `key`, where two different causes merge into a write.
pub(super) fn merge(causes: &mut Causes, key: ChunkKey, cause: Cause) {
    let held = causes.get_or_insert_with(key, || cause);
    if *held != cause {
        *held = Cause::Write;
    }
}

/// Adds what `journal` says about each changed chunk key to `keys`.
fn key_changes(journal: &Journal, keys: &mut Keys) {
    for (position, change) in journal.chunks.iter().enumerate() {
        match change {
            ChunkChange::Replaced { key, .. } => {
                let state = keys.get_or_insert_with(*key, KeyChanges::default);
                state.first_replaced.get_or_insert(position);
                state.recorded = true;
            }
            ChunkChange::Props { key, .. } => {
                keys.get_or_insert_with(*key, KeyChanges::default).props = true;
            }
            ChunkChange::Offset { key, .. } => {
                keys.get_or_insert_with(*key, KeyChanges::default);
            }
        }
    }
}

/// The most emptied derived states the scene keeps for new chunk records.
const SPARE_DERIVED: usize = 64;

/// Empties the recordings and derived states of replaced and removed chunks and moves them into
/// the spare pools.
pub(crate) fn recycle(derive: &mut Derive<'_>, journal: &mut Journal) {
    for record in journal.replaced.drain(..).flatten() {
        let mut recorded = record.recorded;
        recorded.clear();
        derive.spare.push(recorded);
        let spare = &mut derive.scene.spare_derived;
        if spare.len() < SPARE_DERIVED {
            let mut derived = record.derived;
            derived.clear();
            spare.push(derived);
        }
    }
}

/// Returns what identifies the lowerings a chunk holds: the revision of its lowering and the
/// glyph runs kept lowered for it.
fn lowered_parts(scene: &Scene, key: ChunkKey) -> Option<(Option<u64>, *const RunLowering, usize)> {
    let derived = &scene.chunks.get(&key)?.derived;
    let runs = &derived.lowered_runs;
    Some((
        derived.lowered.as_ref().map(|lowered| lowered.revision),
        runs.as_ptr().cast(),
        runs.len(),
    ))
}
