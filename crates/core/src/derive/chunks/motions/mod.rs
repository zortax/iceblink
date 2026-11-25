//! The chunks a commit moved through the spaces and clips they refer to.

mod clips;
mod take;
mod verify;

use iceblink::ChunkKey;

use self::clips::walk_clips;
use self::take::Taker;
use crate::derive::Derive;
use crate::derive::chunks::marks::Marks;
use crate::derive::chunks::restill::{LastChange, Restilled};
use crate::derive::chunks::{Causes, Changes, Keys, merge};
use crate::derive::links::Cause;
use crate::scene::Scene;
use crate::scene::follows::Follows;
use crate::scene::kind::Kind;

/// The chunks and counts found while finding the moved chunks.
#[derive(Debug, Default)]
pub(super) struct Walked {
    /// Users of changed clips checked one by one.
    pub(super) checked: u64,
    /// Users left out or restilled by their stored reach that a check one by one would treat
    /// otherwise, and clips whose reaches miss a user, found under verification.
    pub(super) mismatches: u64,
    /// Written clips that left many users alone or restilled them quietly, and keep no reaches
    /// yet.
    pub(super) unkept: Vec<u32>,
    /// Chunks with local clips that a clip write left drawing the same pixels.
    pub(super) kept: Vec<ChunkKey>,
    /// Carried chunks whose still pixels alone a clip write changed, to be restilled one by one.
    pub(super) stills: Vec<ChunkKey>,
    /// Carried chunks restilled with no damage.
    pub(super) quiet: Vec<Quiet>,
}

impl Walked {
    /// Empties the lists, keeping their capacity, and zeroes the counts.
    fn clear(&mut self) {
        self.checked = 0;
        self.mismatches = 0;
        self.unkept.clear();
        self.kept.clear();
        self.stills.clear();
        self.quiet.clear();
    }
}

/// The lists finding the moved chunks fills, kept empty between commits.
#[derive(Debug, Default)]
pub(crate) struct WalkedLists(Walked);

impl WalkedLists {
    /// Returns the empty lists to fill.
    pub(super) fn take(&mut self) -> Walked {
        core::mem::take(&mut self.0)
    }

    /// Keeps the lists of `walked`, emptied, for the next commit.
    pub(super) fn keep(&mut self, mut walked: Walked) {
        walked.clear();
        self.0 = walked;
    }
}

/// A carried chunk restilled with no damage.
#[derive(Debug)]
pub(super) struct Quiet {
    pub(super) key: ChunkKey,
    pub(super) outcome: Restilled,
    /// How many chunks the motions held when this one was restilled. A chunk the motions take
    /// later is refreshed whole instead.
    pub(super) motions: usize,
}

/// Adds the chunks drawn through a space or clip the commit moved to `motions`, with the cause of
/// the move, and returns what else the walk found.
///
/// The returned lists hold the chunks with local clips that a clip write left drawing the same
/// pixels, and the carried chunks whose still pixels alone a clip write changed, as
/// [`admit`](crate::derive::chunks::restill::admit) decides with `last`.
///
/// Of the spaces and clips that follow one scroll offset alone and that a travel moved, only the
/// users no scroll node carries are taken. A user of a written clip that the commit did not write
/// and that keeps its pixels is left out.
///
/// # Users left out by their reach
///
/// A written clip that keeps its users' reaches decides some users without looking at their
/// records, when the clip's spaces follow the same offsets as before and no space a travel moved
/// has carried users:
///
/// - A plain user whose stored reach the clip cuts alike is left out, when the commit moved no
///   space.
/// - A carried user is restilled quietly as
///   [`admit_carried`](crate::derive::chunks::restill::admit_carried) decides, when the clip moves
///   with no scroll offset and keeps its still root.
///
/// A user the commit wrote is taken by the clips it refers to after the commit, as any written
/// chunk is.
///
/// A written clip that leaves many users alone or restills them quietly without keeping their
/// reaches is listed in [`Walked::unkept`].
pub(super) fn find_motions(
    derive: &Derive<'_>,
    changes: &Changes<'_>,
    keys: &Keys,
    motions: &mut Causes,
    last: &mut LastChange,
    lists: Walked,
) -> Walked {
    let scene = &*derive.scene;
    let mut taker = Taker::new(derive, changes, keys, last, lists);
    move_space_users(scene, changes, motions, &mut taker.moving);

    // Every user of a moved space is in the motions, unless a travel moved the space and a scroll
    // node carries the user.
    let carried_still = changes.world.moved.values().all(|moved| {
        scene.transforms.at(moved.slot).is_none_or(|space| {
            !travelled(changes, moved.cause, space.follows)
                || space.links.users.len() == space.loose.len()
        })
    });
    let unkept = walk_clips(&mut taker, motions, carried_still);

    move_changed_chunks(changes, motions);

    let (mut walked, shortcuts) = taker.finish();
    walked.unkept = unkept;
    walked.mismatches = shortcuts.mismatches(scene, changes, keys, motions);
    walked
}

/// Reports whether a travel moved an object that follows `follows`, for a move with `cause`.
fn travelled(changes: &Changes<'_>, cause: Cause, follows: Follows) -> bool {
    // The cause is the first scroll that moved a node, which need not be the one it follows.
    cause != Cause::Write
        && matches!(follows, Follows::Scroll(scroll) if changes.travels.moved(scroll))
}

/// Adds the users of every moved space to `motions` and to `moving`.
///
/// Of a space that a travel moved, only the users no scroll node carries are added.
fn move_space_users(
    scene: &Scene,
    changes: &Changes<'_>,
    motions: &mut Causes,
    moving: &mut Marks,
) {
    for moved in changes.world.moved.values() {
        let Some(record) = scene.transforms.at(moved.slot) else {
            continue;
        };
        let users = if travelled(changes, moved.cause, record.follows) {
            &record.loose
        } else {
            &record.links.users
        };
        for &key in users {
            merge(motions, key, moved.cause);
            moving.add(key);
        }
    }
}

/// Adds every changed chunk to `motions` with the cause of each moved space or clip it refers to
/// after the commit.
fn move_changed_chunks(changes: &Changes<'_>, motions: &mut Causes) {
    for net in changes.nets {
        for object in net.after.iter().flatten() {
            let cause = match object.kind {
                Kind::Transform => changes
                    .world
                    .moved
                    .get(&object.slot)
                    .map(|moved| moved.cause),
                Kind::Clip => changes.clips.changed.get(&object.slot).copied(),
                _ => None,
            };
            if let Some(cause) = cause {
                merge(motions, net.key, cause);
            }
        }
    }
}
