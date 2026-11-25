//! The frame clip table as vector planning reads and extends it.

use iceblink::{ClipEntry, FRAME_NO_CLIP, FrameClipShape};

use crate::plan::ClipChains;
use crate::tables::chains::ChainIntern;
use crate::tables::table::FrameTable;

/// The clip chains of one frame's plan.
pub(super) struct Chains<'a> {
    pub(super) clips: &'a mut FrameTable<ClipEntry>,
    pub(super) intern: &'a mut ChainIntern,
    /// The build serial of the frame being planned.
    pub(super) serial: u64,
}

impl ClipChains for Chains<'_> {
    fn parent(&self, clip: u32) -> u32 {
        self.intern.parent(clip)
    }

    fn has_mask(&self, clip: u32) -> bool {
        self.clips.get(clip).is_some_and(|entry| {
            entry
                .links
                .iter()
                .any(|link| matches!(link.shape, FrameClipShape::Mask { .. }))
        })
    }

    fn residual(&mut self, clip: u32, ancestor: u32) -> u32 {
        if ancestor == FRAME_NO_CLIP {
            return clip;
        }
        let skip = self
            .clips
            .get(ancestor)
            .map_or(0, |entry| entry.links.len());
        let links = self
            .clips
            .get(clip)
            .map(|entry| entry.links.iter().skip(skip).cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        self.intern.intern(self.clips, &links, self.serial)
    }
}
