//! Rounded rectangles with a fill and a border.

use bytemuck::{Pod, Zeroable};

use crate::bits::enumeration;
use crate::geometry::LocalRect;
use crate::item::{ClipRef, CornerShapes, SpaceRef};
use crate::paint::PaintRef;

/// A rounded rectangle with a fill and a border.
///
/// The fill covers the whole outer shape, and the border is painted over it. The ink is `bounds`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Quad {
    /// The outer edge.
    pub bounds: LocalRect,
    /// A horizontal and a vertical radius per corner, clockwise from the top left.
    pub radii: [f32; 8],
    /// The corner exponents.
    pub shapes: CornerShapes,
    /// The border widths, as `[top, right, bottom, left]`.
    pub border: [f32; 4],
    /// The paint inside the outer edge.
    pub fill: PaintRef,
    /// The paint of the border band.
    pub stroke: PaintRef,
    /// The offset subtracted from the local point before either paint is evaluated.
    pub paint_origin: [f32; 2],
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
    /// The dash pattern of the whole border.
    pub style: BorderStyle,
    /// Zero.
    pub _pad: [u8; 3],
}

enumeration! {
    /// The dash pattern of a quad's border.
    pub struct BorderStyle(u8) {
        /// One continuous band.
        SOLID = 0,
        /// Dashes twice as long as the side's width, with gaps as long as the width.
        DASHED = 1,
        /// Dashes as long as the side's width, with gaps as long as the width.
        DOTTED = 2,
    }
}
