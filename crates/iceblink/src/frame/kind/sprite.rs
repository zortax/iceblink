//! Sprites over atlas pages.

use bytemuck::{Pod, Zeroable};

use crate::bits::bit_set;
use crate::geometry::LocalRect;
use crate::item::CornerShapes;
use crate::paint::PaintRef;

/// A rectangle of an atlas page, in texels.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct AtlasTile {
    /// The atlas page the tile lies in.
    pub page: u32,
    /// The rectangle, as `[x, y, width, height]`.
    pub rect: [u32; 4],
}

/// A coverage sprite: a glyph mask or a path mask.
///
/// The sprite stretches the coverage of its tile over `bounds` and paints
/// `paint(p - paint_origin) × coverage × clip`. A mono sprite holds one coverage per texel. A
/// subpixel sprite holds one per colour stripe, in the order the renderer was given, and needs
/// dual-source blending. The coverage is corrected for contrast and gamma before it is applied,
/// as described below.
///
/// The compositor places glyph sprites of the atlas route, and path masks under a world matrix
/// with no turn and positive scales, so that `bounds` maps to whole device pixels at one texel per
/// pixel. Such a sprite is drawn without resampling. Every other sprite is sampled bilinearly.
///
/// # Coverage correction
///
/// The correction of a coverage `c` under a paint whose straight colour at the point is
/// `(r, g, b)` takes four steps, with `k` 0.5 for a mono sprite and 1 for each stripe of a
/// subpixel sprite:
///
/// 1. The brightness is `l = 0.30 r + 0.59 g + 0.11 b`.
/// 2. The contrast is `k' = k × clamp(4 (0.75 - l), 0, 1)`.
/// 3. The enhanced coverage is `e = c (k' + 1) / (c k' + 1)`.
/// 4. With `[g0, g1, g2, g3]` the ratios
///    [`gamma_correction_ratios`](crate::color::gamma_correction_ratios) returns for gamma 2.2,
///    the result is `e + e (1 - e) ((g0 v + g1) e + g2 v + g3)`, where `v` is `l` for a mono
///    sprite and the stripe's own channel for a subpixel sprite.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Sprite {
    /// Where the tile is drawn.
    pub bounds: LocalRect,
    /// The fill.
    pub paint: PaintRef,
    /// The offset subtracted from the local point before the paint is evaluated.
    pub paint_origin: [f32; 2],
    /// The coverage.
    pub tile: AtlasTile,
    /// The frame clip slot.
    pub clip: u32,
    /// The frame space slot.
    pub space: u32,
    /// Zero.
    pub _pad: u32,
}

/// A full-colour sprite: a colour glyph or an image.
///
/// The sprite is drawn as an `Image` item is, with `tile` as the source rectangle on its page and
/// the flags standing for the image's flags, extend and filter. The paint is evaluated at
/// `p - paint_origin`. The tile's texels are premultiplied. A colour glyph is a colour sprite with
/// `frame` equal to `bounds`, zero radii, the run's paint, opacity 1 and only `PAINT_ALPHA` set.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ColorSprite {
    /// Where one copy of the tile lands.
    pub bounds: LocalRect,
    /// The rectangle the picture is cut to. The radii are measured against it.
    pub frame: LocalRect,
    /// A horizontal and a vertical radius per corner of `frame`, clockwise from the top left.
    pub radii: [f32; 8],
    /// The corner exponents of `frame`.
    pub shapes: CornerShapes,
    /// The texels.
    pub tile: AtlasTile,
    /// The tint of a mask, or the paint whose alpha scales the result.
    pub paint: PaintRef,
    /// The offset subtracted from the local point before the paint is evaluated.
    pub paint_origin: [f32; 2],
    /// A multiplier on the result, clamped to `[0, 1]`.
    pub opacity: f32,
    /// The frame clip slot.
    pub clip: u32,
    /// The frame space slot.
    pub space: u32,
    /// How the texels are turned into colour.
    pub flags: SpriteFlags,
}

bit_set! {
    /// How a colour sprite's texels are turned into colour.
    pub struct SpriteFlags(u32) {
        /// Desaturated drawing.
        GRAYSCALE = 1 << 0,
        /// The texels' alpha used as coverage for the paint.
        MASK = 1 << 1,
        /// Horizontal repetition of the tile past `bounds`.
        REPEAT_X = 1 << 2,
        /// Vertical repetition of the tile past `bounds`.
        REPEAT_Y = 1 << 3,
        /// Nearest-texel sampling.
        NEAREST = 1 << 4,
        /// The paint's alpha at the point as a multiplier on the result, never set with `MASK`.
        PAINT_ALPHA = 1 << 5,
    }
}
