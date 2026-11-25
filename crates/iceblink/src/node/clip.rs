//! Clip nodes.

use crate::bits::enumeration;
use crate::geometry::LocalRect;
use crate::id::{ClipId, ImageId, TransformId};
use crate::item::CornerShapes;

/// A link of a clip chain.
///
/// A chain is a node and all its ancestors. Its coverage at a pixel is the product of each link's
/// coverage, with each link evaluated in its own space.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipNode {
    /// The next link outwards, or `None` for the outermost link.
    pub parent: Option<ClipId>,
    /// The transform node the shape is measured in.
    pub space: TransformId,
    /// The region the link admits.
    pub shape: ClipShape,
}

/// The region a clip link admits.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipShape {
    /// A rounded rectangle.
    RoundedRect {
        /// The rectangle whose corners are rounded.
        rect: LocalRect,
        /// A horizontal and a vertical radius per corner, clockwise from the top left.
        radii: [f32; 8],
        /// The corner exponents.
        shapes: CornerShapes,
    },
    /// The alpha or luminance of an image, zero outside `rect`.
    Mask {
        /// The image sampled.
        image: ImageId,
        /// The rectangle `source` is mapped onto.
        rect: LocalRect,
        /// The source rectangle, in texels of the image.
        source: [f32; 4],
        /// Which texel values make up the coverage.
        mode: MaskMode,
    },
}

enumeration! {
    /// The texel values a mask clip reads as coverage.
    pub struct MaskMode(u8) {
        /// The texel's alpha.
        ALPHA = 0,
        /// The luminance of the straight colour, times the alpha.
        LUMINANCE = 1,
    }
}
