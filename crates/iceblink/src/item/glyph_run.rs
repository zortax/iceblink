//! Positioned glyphs of one font.

use bytemuck::{Pod, Zeroable};

use crate::bits::bit_set;
use crate::font::FontId;
use crate::item::{ArenaRange, ClipRef, SpaceRef};
use crate::paint::PaintRef;

/// Positioned glyphs of one font at one size, filled with one paint.
///
/// The ink is the union of the glyphs' raster bounds on the device, which the compositor computes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GlyphRun {
    /// The font every glyph is drawn from.
    pub font: FontId,
    /// The em size, in local units.
    pub size: f32,
    /// A range of the chunk's glyph arena.
    pub glyphs: ArenaRange,
    /// A range of the chunk's coordinate arena, one normalised coordinate per variation axis in
    /// the font's axis order.
    ///
    /// An empty range is the font's default instance.
    pub coords: ArenaRange,
    /// The fill. Solid paints can take the atlas route, and other paints take the path route.
    pub paint: PaintRef,
    /// The synthetic emboldening, in device pixels at the rasterised size.
    pub embolden: f32,
    /// The synthetic slant, in degrees, positive leaning forwards.
    pub skew: f32,
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
    /// How the glyphs are rasterised on the atlas route.
    pub flags: GlyphFlags,
    /// Zero.
    pub _pad: [u8; 1],
    /// The colour palette index for colour glyphs.
    pub palette: u16,
}

/// One glyph of a run: its id in the font, and its origin on the baseline in local space.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Glyph {
    /// The glyph's id in the font.
    pub id: u32,
    /// The horizontal position of the origin.
    pub x: f32,
    /// The vertical position of the origin.
    pub y: f32,
}

bit_set! {
    /// How a glyph run is rasterised on the atlas route.
    pub struct GlyphFlags(u8) {
        /// Hinted outlines.
        HINT = 1 << 0,
        /// Per-channel coverage where the output supports it.
        SUBPIXEL = 1 << 1,
        /// A silhouette of the run, such as a text shadow, in which colour glyphs draw nothing.
        SILHOUETTE = 1 << 2,
    }
}
