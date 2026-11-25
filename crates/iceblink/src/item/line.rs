//! Decoration lines.

use bytemuck::{Pod, Zeroable};

use crate::bits::enumeration;
use crate::geometry::LocalRect;
use crate::item::{ClipRef, SpaceRef};
use crate::premultiplied::Color;

/// A decoration line: an underline, an overline or a line-through.
///
/// The ink is `bounds`. The bounds of a wavy or a double line are tall enough for its shape.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Line {
    /// The rectangle the line is drawn in.
    pub bounds: LocalRect,
    /// The colour of the line.
    pub color: Color,
    /// The stroke thickness in local units. Values below 1 draw at 1.
    pub thickness: f32,
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
    /// The shape of the line.
    pub style: LineStyle,
    /// Zero.
    pub _pad: [u8; 3],
}

enumeration! {
    /// The shape of a decoration line.
    pub struct LineStyle(u8) {
        /// One straight band filling the bounds.
        SOLID = 0,
        /// A sine wave about the middle of the bounds.
        WAVY = 1,
        /// Dashes three times as long as the thickness.
        DASHED = 2,
        /// Dashes as long as the thickness.
        DOTTED = 3,
        /// Two bands, each a third of the bounds' height.
        DOUBLE = 4,
    }
}
