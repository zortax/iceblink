//! Rounded rectangles whose pixels an application shader decides.

use bytemuck::{Pod, Zeroable};

use crate::geometry::LocalRect;
use crate::item::{ClipRef, CornerShapes, SpaceRef};
use crate::paint::PaintRef;

/// A rounded rectangle whose pixels an application shader decides.
///
/// The shader's mode decides which fields are read:
///
/// - In paint mode, the shader returns the colour inside the rounded rectangle, and the paints
///   and border widths are not read.
/// - In coverage mode, the shader returns the coverage of the box and of its border, which the
///   fill and the stroke paint. The radii and exponents are not read.
///
/// The ink is `bounds`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ShaderQuad {
    /// The outer edge.
    pub bounds: LocalRect,
    /// A horizontal and a vertical radius per corner, clockwise from the top left.
    pub radii: [f32; 8],
    /// The corner exponents.
    pub shapes: CornerShapes,
    /// The border widths, as `[top, right, bottom, left]`.
    pub border: [f32; 4],
    /// The paint inside the shader's coverage.
    pub fill: PaintRef,
    /// The paint of the shader's border coverage.
    pub stroke: PaintRef,
    /// The offset subtracted from the local point before either paint is evaluated.
    pub paint_origin: [f32; 2],
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
    /// The slot of a shader in paint or coverage mode.
    pub shader: u32,
    /// The slot of the shader's parameter block.
    pub params: u32,
    /// A multiplier on the result, clamped to `[0, 1]`.
    pub opacity: f32,
}
