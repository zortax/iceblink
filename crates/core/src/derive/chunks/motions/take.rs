//! The decision whether a user of a changed clip moves.

use iceblink::ChunkKey;

use super::verify::Shortcuts;
use super::{Quiet, Walked};
use crate::derive::Derive;
use crate::derive::chunks::kept::keeps_pixels;
use crate::derive::chunks::marks::Marks;
use crate::derive::chunks::restill::{Admitted, LastChange, admit, admit_carried};
use crate::derive::chunks::{Causes, Changes, Keys, merge};
use crate::derive::links::Cause;
use crate::geometry::coverage::CoverageChange;
use crate::scene::Scene;
use crate::scene::reaches::CarriedReach;

/// The users of changed clips taken so far, and the chunks found among them.
pub(super) struct Taker<'a> {
    pub(super) scene: &'a Scene,
    pub(super) changes: &'a Changes<'a>,
    keys: &'a Keys,
    last: &'a mut LastChange,
    pub(super) verification: bool,
    /// The chunks the commit wrote.
    written: Marks,
    /// The chunks added to the motions.
    pub(super) moving: Marks,
    walked: Walked,
    /// The users decided by their stored reach, listed under verification.
    pub(super) shortcuts: Shortcuts,
}

impl<'a> Taker<'a> {
    pub(super) fn new(
        derive: &'a Derive<'_>,
        changes: &'a Changes<'a>,
        keys: &'a Keys,
        last: &'a mut LastChange,
        walked: Walked,
    ) -> Self {
        // Most users are in neither map, which the marks tell without a lookup.
        let mut written = Marks::default();
        for (&key, _) in keys.iter() {
            written.add(key);
        }
        Self {
            scene: &*derive.scene,
            changes,
            keys,
            last,
            verification: derive.verification,
            written,
            moving: Marks::default(),
            walked,
            shortcuts: Shortcuts::default(),
        }
    }

    /// Decides what happens to user `key` of a clip changed with `cause`, and reports whether
    /// the user keeps its pixels or restills quietly.
    ///
    /// A user of a clip a scroll moved, and a user the commit wrote or already moved, is added to
    /// `motions`. Any other user is decided in this order:
    ///
    /// - If it restills quietly, from its `stored` reach when it has one, it is listed as quiet.
    /// - If it must be restilled one by one, it is listed in the stills.
    /// - If it keeps its pixels, it is listed in the kept chunks when it has local clips.
    /// - Otherwise it is added to `motions` with `cause`.
    pub(super) fn take(
        &mut self,
        key: ChunkKey,
        cause: Cause,
        motions: &mut Causes,
        stored: Option<(&CoverageChange, CarriedReach)>,
    ) -> bool {
        let scene = self.scene;
        let changes = self.changes;
        let fresh = cause == Cause::Write
            && !(self.written.may_hold(key) && self.keys.contains_key(&key))
            && !(self.moving.may_hold(key) && motions.contains_key(&key));
        let quiet = |outcome| Quiet {
            key,
            outcome,
            motions: motions.len(),
        };

        if fresh
            && let Some((change, carried)) = stored
            && let Some(outcome) = admit_carried(scene, change, carried, changes, self.last)
        {
            self.walked.quiet.push(quiet(outcome));
            if self.verification {
                self.shortcuts.quiet.push((key, outcome));
            }
            return true;
        }
        self.walked.checked += 1;
        let untouched = fresh.then(|| scene.chunks.get(&key)).flatten();
        if let Some(record) = untouched {
            match admit(scene, record, changes, self.last) {
                Admitted::Quiet(outcome) => {
                    self.walked.quiet.push(quiet(outcome));
                    return true;
                }
                Admitted::Restill => {
                    self.walked.stills.push(key);
                    return false;
                }
                Admitted::No => {}
            }
            if keeps_pixels(scene, record, changes) {
                if !record.derived.local_frames.is_empty() {
                    self.walked.kept.push(key);
                }
                return true;
            }
        }
        merge(motions, key, cause);
        self.moving.add(key);
        false
    }

    /// Returns what the taken users found, and the users decided by their stored reach under
    /// verification.
    pub(super) fn finish(self) -> (Walked, Shortcuts) {
        (self.walked, self.shortcuts)
    }
}
