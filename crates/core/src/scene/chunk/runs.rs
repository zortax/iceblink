//! The glyph runs of a chunk as last lowered and measured.

use iceblink::{DeviceRect, LocalRect, Matrix4, PaintRef};

use crate::content::{Lowering as ContentLowering, Target, keeps_pixels};
use crate::geometry::bounds::after_offset;

/// A glyph run lowered with frame slots, and the inputs it was lowered with.
#[derive(Debug)]
pub(crate) struct RunLowering {
    pub(crate) target: Target,
    /// Whether the run could draw per-channel coverage.
    pub(crate) subpixel: bool,
    /// The output scale as `f32` bits.
    pub(crate) scale_bits: u32,
    /// The run's paint, with its frame paint slot.
    pub(crate) paint: PaintRef,
    /// The content generation the tiles belong to.
    pub(crate) epoch: u64,
    /// The evictions the content cache had made.
    pub(crate) evictions: u64,
    pub(crate) lowering: ContentLowering,
}

/// The ink of a glyph run, measured on its lowered glyphs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RunInk {
    /// A local rectangle that holds the glyphs under every translation of the world matrix, or
    /// `None` for a run that draws nothing.
    pub(crate) local: Option<LocalRect>,
    /// The device pixels the glyphs cover under `world` at `offset`.
    pub(crate) device: Option<DeviceRect>,
    /// The world matrix the glyphs were lowered under.
    pub(crate) world: Matrix4,
    /// The chunk offset the glyphs were lowered at.
    pub(crate) offset: [f32; 2],
    /// Whether the world matrix was moving when the glyphs were lowered.
    pub(crate) moving: bool,
}

impl RunInk {
    /// Returns how far the glyphs move under `world` at chunk offset `offset`, in whole device
    /// pixels, or `None` when they would land on other pixels or phases than where they were
    /// measured.
    pub(crate) fn moved_by(&self, world: &Matrix4, offset: [f32; 2]) -> Option<[f32; 2]> {
        if *world == self.world && offset == self.offset {
            return Some([0.0; 2]);
        }

        let before = after_offset(&self.world, self.offset);
        let after = after_offset(world, offset);
        if !keeps_pixels(&before, &after) {
            return None;
        }

        let (before, after) = (before.to_affine2()?, after.to_affine2()?);
        Some([after.tx - before.tx, after.ty - before.ty])
    }
}
