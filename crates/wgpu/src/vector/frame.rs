//! The work a rasteriser draws for one round of a frame.

use core::ops::Range;

use iceblink::{
    ClipEntry, ClipLinkEntry, FRAME_NO_CLIP, FramePaint, FramePath, Matrix4, PlannedPath,
};

use crate::vector::resource::ResourcedPlan;

/// One round of a frame's vector work, with the tables its paths resolve through.
///
/// The tables are the renderer's complete tables after the frame's deltas are applied.
pub struct VectorFrame<'frame> {
    /// The resourced plan of the frame.
    pub plan: &'frame ResourcedPlan,
    /// The frame's paths, which the plan's items index.
    pub paths: &'frame [FramePath],
    /// The clip chains that pass clips and residual clips index.
    pub clips: &'frame [ClipEntry],
    /// The paints that fills and strokes index.
    pub paints: &'frame [FramePaint],
    /// The device matrices that clip links index.
    pub spaces: &'frame [Matrix4],
    /// The round of the plan to rasterise.
    pub round: usize,
    /// The wgpu device.
    pub device: &'frame wgpu::Device,
    /// The wgpu queue.
    pub queue: &'frame wgpu::Queue,
}

impl VectorFrame<'_> {
    /// Returns the indices of the passes of this round.
    pub fn passes(&self) -> Range<usize> {
        self.plan.round(self.round)
    }

    /// Reports whether this round has no passes.
    pub fn is_empty(&self) -> bool {
        self.passes().is_empty()
    }

    /// Returns the frame path `planned` draws, or `None` for an index past the frame's paths.
    pub fn path(&self, planned: &PlannedPath) -> Option<&FramePath> {
        self.paths.get(planned.path as usize)
    }

    /// Returns the links of the residual clip of `planned`, outermost first.
    ///
    /// The links are empty for a path drawn with no residual, whose slot is `FRAME_NO_CLIP`, and
    /// for a slot past the clip table.
    pub fn residual(&self, planned: &PlannedPath) -> &[ClipLinkEntry] {
        if planned.residual == FRAME_NO_CLIP {
            return &[];
        }
        self.clips
            .get(planned.residual as usize)
            .map_or(&[], |entry| entry.links.as_slice())
    }
}
