//! The geometry of marks items.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

/// The geometry of a marks item, shared by every item and frame that draws it.
#[derive(Clone, Debug, Default)]
pub struct MarkPayload {
    /// Discs and rings, as `[cx, cy, outer, inner]`.
    pub discs: Vec<[f32; 4]>,
    /// Rounded boxes.
    pub boxes: Vec<MarkBox>,
    /// Polyline vertices.
    ///
    /// A vertex with a NaN `x` separates runs, and one comes first and last.
    pub vertices: Vec<[f32; 2]>,
    /// Outlines, in device pixels with y down and the anchor at the origin.
    pub shapes: Vec<Arc<kurbo::BezPath>>,
    /// Outlines placed at payload positions.
    pub placements: Vec<MarkPlacement>,
}

/// A rounded box of a marks payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MarkBox {
    /// The outer edge, as `[x0, y0, x1, y1]`.
    pub rect: [f32; 4],
    /// A horizontal and a vertical radius per corner, clockwise from the top left.
    pub radii: [f32; 8],
    /// The corner exponent, the border width, and two zeros.
    ///
    /// A border width above zero draws only the band of that width inside the edge.
    pub shape: [f32; 4],
}

/// An outline of a marks payload placed at a payload position.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct MarkPlacement {
    /// The horizontal payload position of the anchor.
    pub x: f32,
    /// The vertical payload position of the anchor.
    pub y: f32,
    /// The outline's index in `MarkPayload::shapes`.
    pub shape: u32,
    /// Zero.
    pub _pad: u32,
}
