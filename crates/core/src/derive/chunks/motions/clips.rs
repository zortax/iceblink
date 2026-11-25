//! The users of the clips a commit changed.

use super::take::Taker;
use super::travelled;
use crate::derive::chunks::Causes;
use crate::derive::chunks::kept::cuts_alike;
use crate::derive::clips::ChainBefore;
use crate::derive::links::Cause;
use crate::scene::follows::Follows;
use crate::scene::reaches::Reaches;
use crate::scene::record::ClipRecord;

/// The fewest users that keep their pixels or restill quietly under a write of a clip before the
/// clip keeps their reaches.
// Keeping reaches costs every chunk that refers to the clip on each new ink, so only clips whose
// writes leave many users alone keep them.
const KEPT_FROM: usize = 32;

/// Takes the users of every changed clip through `taker`, which adds those that move to
/// `motions`.
///
/// `carried_still` is set when no space a travel moved has carried users.
///
/// Returns the written clips that left many users alone or restilled them quietly, and keep no
/// reaches yet.
pub(super) fn walk_clips(
    taker: &mut Taker<'_>,
    motions: &mut Causes,
    carried_still: bool,
) -> Vec<u32> {
    let scene = taker.scene;
    let changes = taker.changes;
    let mut unkept = Vec::new();
    for (&clip, &cause) in &changes.clips.changed {
        let Some(record) = scene.clips.at(clip) else {
            continue;
        };
        let alike = cause == Cause::Write
            && carried_still
            && changes
                .clips
                .before
                .get(&clip)
                .is_some_and(|before| before.follows == record.split.follows);
        if alike
            && let Some(reaches) = &record.reaches
            && let Some(before) = changes.clips.before.get(&clip)
        {
            walk_reaches(taker, motions, clip, record, before, reaches, cause);
            continue;
        }
        let users = if travelled(changes, cause, record.split.follows) {
            &record.loose
        } else {
            &record.links.users
        };
        let mut alone = 0;
        for &key in users {
            alone += usize::from(taker.take(key, cause, motions, None));
        }
        if alike && alone >= KEPT_FROM {
            unkept.push(clip);
        }
    }
    unkept
}

/// Takes the users of written clip `record` at slot `clip` by their stored `reaches`, and leaves
/// out the plain users the clip cuts alike when no space moved.
fn walk_reaches(
    taker: &mut Taker<'_>,
    motions: &mut Causes,
    clip: u32,
    record: &ClipRecord,
    before: &ChainBefore,
    reaches: &Reaches,
    cause: Cause,
) {
    let changes = taker.changes;
    let coverage = changes.clips.coverage.get(&clip);
    if taker.verification && reaches.users().len() != record.links.users.len() {
        taker.shortcuts.incomplete += 1;
    }
    let restills = (before.follows == Follows::Still && record.split.root == before.root)
        .then_some(coverage)
        .flatten();
    let world_still = changes.world.moved.len() == 0;
    for user in reaches.users() {
        if world_still && user.plain && cuts_alike(before, record, coverage, user.reach) {
            if taker.verification {
                taker.shortcuts.left_out.push(user.key);
            }
        } else {
            let stored = restills.zip(user.carried);
            taker.take(user.key, cause, motions, stored);
        }
    }
}
