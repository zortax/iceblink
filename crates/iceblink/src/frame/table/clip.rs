//! Clip chains in a frame.

use smallvec::SmallVec;

use crate::frame::AtlasTile;
use crate::geometry::LocalRect;
use crate::item::CornerShapes;
use crate::node::MaskMode;

/// A clip chain, as the links an instance is drawn through.
///
/// The chain's coverage at a device pixel is the product of every link's coverage. Each link is
/// measured in its own space, so moving a space changes the space table and leaves the clip
/// table as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipEntry {
    /// The links, outermost first.
    pub links: SmallVec<[ClipLinkEntry; 2]>,
}

/// One link of a clip chain.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipLinkEntry {
    /// The frame space slot the shape is measured in.
    pub space: u32,
    /// The shape that clips, measured in `space`.
    pub shape: FrameClipShape,
}

/// The shape of a clip link, in the link's space.
///
/// Coverage follows the clip rules of `ClipNode`. A rounded rectangle with all radii zero whose
/// world matrix maps it to an axis-aligned device rectangle is a hard edge.
#[derive(Clone, Debug, PartialEq)]
pub enum FrameClipShape {
    /// A rounded rectangle.
    RoundedRect {
        /// The rectangle whose corners are rounded.
        rect: LocalRect,
        /// A horizontal and a vertical radius per corner, clockwise from the top left.
        radii: [f32; 8],
        /// The corner exponents.
        shapes: CornerShapes,
    },
    /// A mask image stretched over a rectangle.
    Mask {
        /// The rectangle the mask covers. Coverage is zero outside it.
        rect: LocalRect,
        /// The part of the image's tile that the client's `source` rectangle covers, rounded to
        /// whole texels.
        tile: AtlasTile,
        /// How texels are turned into coverage.
        mode: MaskMode,
    },
}
