//! The draw sequence of a frame.

use core::ops::Range;

use crate::geometry::DeviceRect;

/// One step of a frame's draw sequence.
///
/// A renderer draws the batches in sequence. For an instanced kind, `range` is a range of
/// positions in that lane, counting the instances of its runs, and the batch draws those
/// instances in lane order. Every other kind draws the object at its index, and its range is
/// empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    /// What the batch draws.
    pub kind: BatchKind,
    /// The instances drawn, as a range of positions in the lane.
    pub range: Range<u32>,
    /// The device pixels of its target the batch can change.
    ///
    /// Inside a group the target covers the group's source, so the ink can lie past the output. A
    /// renderer may leave out an instanced batch in a region its ink misses.
    pub ink: DeviceRect,
}

/// What a batch draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchKind {
    /// Quads.
    Quads,
    /// Shader quads, all with this frame shader slot and parameter slot.
    ShaderQuads {
        /// The frame shader slot.
        shader: u32,
        /// The frame parameter slot.
        params: u32,
    },
    /// Shadows.
    Shadows,
    /// Lines.
    Lines,
    /// Mono sprites, all reading this atlas page.
    Mono {
        /// The atlas page the sprites read.
        page: u32,
    },
    /// Subpixel sprites, all reading this atlas page.
    Subpixel {
        /// The atlas page the sprites read.
        page: u32,
    },
    /// Colour sprites, all reading this atlas page.
    Color {
        /// The atlas page the sprites read.
        page: u32,
    },
    /// Marks.
    Marks,
    /// The composite of the vector pass at this index of the vector plan.
    Paths {
        /// The pass's index in the vector plan.
        pass: u32,
    },
    /// The external quad at this index of `Frame::externals`.
    External(u32),
    /// The backdrop at this index of `Frame::backdrops`, applied to the target open at this point.
    Backdrop(u32),
    /// The start of the group at this index of `Frame::groups`, which redirects drawing into a
    /// cleared target.
    GroupBegin(u32),
    /// The end of the group at this index, which composites its target into the target beneath
    /// it.
    GroupEnd(u32),
}
