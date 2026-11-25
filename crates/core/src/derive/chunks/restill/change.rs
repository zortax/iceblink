//! Where the changed clips a carried chunk refers to cover differently.

use smallvec::SmallVec;

use crate::derive::clips::ClipChanges;
use crate::geometry::coverage::CoverageChange;
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;
use crate::scene::inks::Still;
use crate::scene::kind::Kind;

/// The changed clips the last restilled chunk refers to, and where their chains cover
/// differently, kept for the next chunk that refers to the same clips.
#[derive(Default)]
pub(in crate::derive::chunks) struct LastChange {
    clips: SmallVec<[u32; 4]>,
    change: CoverageChange,
    /// The clip node the last carry looked up referred to, and whether its still pixels moved,
    /// or `None` when they are not whole.
    still: Option<(u32, Option<bool>)>,
}

impl LastChange {
    /// Returns where the changed clips chunk `record` refers to cover differently after the
    /// commit.
    pub(super) fn change_of(
        &mut self,
        record: &ChunkRecord,
        clips: &ClipChanges,
    ) -> &CoverageChange {
        let changed: SmallVec<[u32; 4]> = record
            .derived
            .deps
            .iter()
            .filter(|object| object.kind == Kind::Clip && clips.coverage.contains_key(&object.slot))
            .map(|object| object.slot)
            .collect();
        self.change(&changed, clips)
    }

    /// Returns where the changed clips at slots `changed` cover differently after the commit.
    pub(super) fn change(&mut self, changed: &[u32], clips: &ClipChanges) -> &CoverageChange {
        if self.clips.as_slice() != changed {
            self.clips.clear();
            self.clips.extend_from_slice(changed);
            self.change = CoverageChange::default();
            for slot in &self.clips {
                if let Some(coverage) = clips.coverage.get(slot) {
                    self.change.add(coverage);
                }
            }
        }
        &self.change
    }

    /// Returns whether the still pixels of clip node `root` moved in the commit, or `None` when
    /// they are not whole pixels after it.
    pub(super) fn still_moved(
        &mut self,
        root: u32,
        scene: &Scene,
        clips: &ClipChanges,
    ) -> Option<bool> {
        match self.still {
            Some((held, moved)) if held == root => moved,
            _ => {
                let now = scene.clips.at(root).and_then(|clip| clip.still);
                let moved = now.map(|now| now != clips.still_before(scene, Still::Node(root)));
                self.still = Some((root, moved));
                moved
            }
        }
    }
}
