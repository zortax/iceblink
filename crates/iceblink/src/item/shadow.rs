//! Blurred rounded rectangles cast by a rounded rectangle.

use bytemuck::{Pod, Zeroable};

use crate::geometry::LocalRect;
use crate::item::{ClipRef, CornerShapes, SpaceRef};
use crate::premultiplied::Color;

/// A blurred rounded rectangle cast by another rounded rectangle, the caster.
///
/// The kind of shadow decides where it paints:
///
/// - A drop shadow paints only outside the caster. Its ink is `shape_bounds` outset by
///   `3 × blur` on every side.
/// - An inset shadow paints only inside the caster. Its ink is `caster_bounds`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Shadow {
    /// The shape the blur is applied to.
    pub shape_bounds: LocalRect,
    /// The radii of the blurred shape.
    pub radii: [f32; 8],
    /// The rectangle that casts the shadow.
    pub caster_bounds: LocalRect,
    /// The radii of the caster.
    pub caster_radii: [f32; 8],
    /// The corner exponents of both shapes.
    pub shapes: CornerShapes,
    /// The colour of the fully covered shadow.
    pub color: Color,
    /// The standard deviation of the blur, in local units.
    pub blur: f32,
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
    /// One for an inset shadow, zero for a drop shadow.
    pub inset: u32,
}
