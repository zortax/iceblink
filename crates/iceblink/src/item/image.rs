//! Image resources drawn into a rectangle.

use bytemuck::{Pod, Zeroable};

use crate::bits::{bit_set, enumeration};
use crate::geometry::LocalRect;
use crate::item::{ClipRef, CornerShapes, SpaceRef};
use crate::paint::PaintRef;

/// An image resource drawn into a rectangle.
///
/// The source rectangle is stretched over `bounds` and cut to the rounded rectangle of `frame`.
/// The ink is `bounds ∩ frame`, widened to `frame` along an axis that repeats.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Image {
    /// Where one copy of the source rectangle lands.
    pub bounds: LocalRect,
    /// The rectangle the picture is cut to. The radii are measured against it.
    pub frame: LocalRect,
    /// The source rectangle, in texels of the image.
    pub source: [f32; 4],
    /// A horizontal and a vertical radius per corner of `frame`, clockwise from the top left.
    pub radii: [f32; 8],
    /// The corner exponents of `frame`.
    pub shapes: CornerShapes,
    /// The tint of a mask image.
    pub paint: PaintRef,
    /// The image's slot.
    pub image: u32,
    /// A multiplier on the result, clamped to `[0, 1]`.
    pub opacity: f32,
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
    /// How the texels are turned into colour.
    pub flags: ImageFlags,
    /// How the picture continues past `bounds`, as `[x, y]`.
    pub extend: [Extend; 2],
    /// How texels are sampled.
    pub filter: ImageFilter,
}

bit_set! {
    /// How an image's texels are turned into colour.
    pub struct ImageFlags(u8) {
        /// Desaturated drawing, which a mask ignores.
        GRAYSCALE = 1 << 0,
        /// The image's alpha used as coverage for the item's paint.
        MASK = 1 << 1,
    }
}

enumeration! {
    /// How a picture continues past its bounds along one axis.
    pub struct Extend(u8) {
        /// Transparent outside the bounds.
        NONE = 0,
        /// Repeated copies.
        REPEAT = 1,
    }
}

enumeration! {
    /// How an image's texels are sampled.
    pub struct ImageFilter(u8) {
        /// Bilinear filtering.
        LINEAR = 0,
        /// The nearest texel.
        NEAREST = 1,
    }
}
