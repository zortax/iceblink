//! Verification's record of what each pass key was issued for.

use std::collections::hash_map::Entry;
use std::hash::Hasher;
use std::sync::Arc;

use iceblink::{FramePaint, FramePath, PassKey, PlannedPath, VectorPlan};
use rustc_hash::FxHashMap;

use super::fold::{Outlines, fold_pass};
use crate::tables::table::FrameTable;

/// The digest of the content each pass key was first issued for.
///
/// A digest folds the outlines' elements where the key folds their addresses, so a key issued
/// again for other content shows as a second digest.
#[derive(Debug, Default)]
pub(crate) struct PassDigests {
    digests: FxHashMap<PassKey, Issued>,
}

/// One key's digest, with the outlines its key was folded from.
#[derive(Debug)]
struct Issued {
    digest: u64,
    // Held so no other outline takes their addresses while the key is recorded.
    _outlines: Vec<Arc<kurbo::BezPath>>,
}

impl PassDigests {
    /// The most keys held before the record is cleared.
    const LIMIT: usize = 1 << 16;

    /// Records the digest of every keyed pass of `plan`, and returns how many keyed passes have
    /// a key that was issued before for another digest.
    pub(crate) fn check(
        &mut self,
        plan: &VectorPlan,
        paths: &[FramePath],
        paints: &FrameTable<FramePaint>,
    ) -> u64 {
        if self.digests.len() > Self::LIMIT {
            self.digests.clear();
        }

        let mut reused = 0;
        for pass in plan.passes.iter().filter(|pass| pass.key != PassKey::NONE) {
            let planned = plan.items_of(pass);
            let digest = fold_pass(pass, planned, paths, paints, Outlines::Elements).finish();
            match self.digests.entry(pass.key) {
                Entry::Vacant(vacant) => {
                    vacant.insert(Issued {
                        digest,
                        _outlines: outlines(planned, paths),
                    });
                }
                Entry::Occupied(held) if held.get().digest != digest => {
                    tracing::warn!(
                        key = pass.key.0,
                        "a pass key was issued again for other content"
                    );
                    reused += 1;
                }
                Entry::Occupied(_) => {}
            }
        }
        reused
    }
}

/// Returns the outlines and clip outlines of the paths of `planned`.
fn outlines(planned: &[PlannedPath], paths: &[FramePath]) -> Vec<Arc<kurbo::BezPath>> {
    planned
        .iter()
        .filter_map(|item| paths.get(item.path as usize))
        .flat_map(|path| {
            core::iter::once(Arc::clone(&path.path))
                .chain(path.clips.iter().map(|clip| Arc::clone(&clip.path)))
        })
        .collect()
}
