//! Checking the users decided by their stored reach one by one, under verification.

use iceblink::ChunkKey;

use crate::derive::chunks::kept::keeps_pixels;
use crate::derive::chunks::restill::{Admitted, LastChange, Restilled, admit};
use crate::derive::chunks::{Causes, Changes, Keys};
use crate::scene::Scene;

/// The users of written clips decided by their stored reach.
#[derive(Default)]
pub(super) struct Shortcuts {
    /// Plain users left out.
    pub(super) left_out: Vec<ChunkKey>,
    /// Carried users restilled quietly, with the outcome.
    pub(super) quiet: Vec<(ChunkKey, Restilled)>,
    /// Clips whose reaches miss a user.
    pub(super) incomplete: u64,
}

impl Shortcuts {
    /// Counts the users that a check one by one treats otherwise, and the clips whose reaches
    /// miss a user.
    pub(super) fn mismatches(
        &self,
        scene: &Scene,
        changes: &Changes<'_>,
        keys: &Keys,
        motions: &Causes,
    ) -> u64 {
        let left_out = self
            .left_out
            .iter()
            .filter(|&&key| !motions.contains_key(&key) && !left_alone(scene, changes, key))
            .count() as u64;
        let quiet = self
            .quiet
            .iter()
            .filter(|&&(key, outcome)| {
                keys.contains_key(&key)
                    || !motions.contains_key(&key)
                        && !restills_quietly(scene, changes, key, outcome)
            })
            .count() as u64;
        self.incomplete + left_out + quiet
    }
}

/// Reports whether a check one by one leaves chunk `key` alone.
#[cold]
fn left_alone(scene: &Scene, changes: &Changes<'_>, key: ChunkKey) -> bool {
    scene.chunks.get(&key).is_some_and(|record| {
        admit(scene, record, changes, &mut LastChange::default()) == Admitted::No
            && keeps_pixels(scene, record, changes)
            && record.derived.local_frames.is_empty()
    })
}

/// Reports whether a check one by one restills chunk `key` quietly with `outcome`.
#[cold]
fn restills_quietly(
    scene: &Scene,
    changes: &Changes<'_>,
    key: ChunkKey,
    outcome: Restilled,
) -> bool {
    scene.chunks.get(&key).is_some_and(|record| {
        admit(scene, record, changes, &mut LastChange::default()) == Admitted::Quiet(outcome)
    })
}
