//! What a rasterised glyph and a glyph outline are cached by.

use crate::font::FontId;

/// Everything that decides a rasterised glyph's pixels.
///
/// Equal keys with equal variation coordinates produce identical bitmaps. Lengths are held as
/// `f32` bits, so the key hashes and compares exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// The font the glyph belongs to.
    pub font: FontId,
    /// The glyph's id in the font.
    pub glyph: u32,
    /// The em size in device pixels, as `f32` bits.
    pub size_bits: u32,
    /// The compositor's interned id of the variation coordinates, zero for the default instance.
    pub instance: u32,
    /// How far the outline is shifted right before rasterising, in quarter pixels from 0 to 3.
    pub phase: u8,
    /// The kind of bitmap.
    pub style: RasterStyle,
    /// Whether the outline is hinted.
    pub hint: bool,
    /// The colour palette of a colour glyph.
    pub palette: u16,
    /// The synthetic emboldening in device pixels, as `f32` bits.
    pub embolden_bits: u32,
    /// The synthetic slant in degrees, positive leaning forwards, as `f32` bits.
    pub skew_bits: u32,
}

impl GlyphKey {
    /// Returns the em size in device pixels.
    pub fn size(&self) -> f32 {
        f32::from_bits(self.size_bits)
    }

    /// Returns the synthetic emboldening in device pixels.
    pub fn embolden(&self) -> f32 {
        f32::from_bits(self.embolden_bits)
    }

    /// Returns the synthetic slant in degrees.
    pub fn skew(&self) -> f32 {
        f32::from_bits(self.skew_bits)
    }
}

/// Everything that decides a glyph's outline.
///
/// The key holds no emboldening. Emboldening is a stroke drawn over the filled outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutlineKey {
    /// The font the glyph belongs to.
    pub font: FontId,
    /// The glyph's id in the font.
    pub glyph: u32,
    /// The em size in device pixels, as `f32` bits.
    pub size_bits: u32,
    /// The compositor's interned id of the variation coordinates, zero for the default instance.
    pub instance: u32,
    /// The synthetic slant in degrees, positive leaning forwards, as `f32` bits.
    pub skew_bits: u32,
}

impl OutlineKey {
    /// Returns the em size in device pixels.
    pub fn size(&self) -> f32 {
        f32::from_bits(self.size_bits)
    }

    /// Returns the synthetic slant in degrees.
    pub fn skew(&self) -> f32 {
        f32::from_bits(self.skew_bits)
    }
}

/// The kind of bitmap a glyph is rasterised to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RasterStyle {
    /// One coverage byte per pixel.
    Mono,
    /// Three coverage bytes per pixel, one per colour stripe, in the order `RedToBlue`.
    Subpixel,
    /// Four bytes per pixel of straight-alpha gamma-encoded sRGB, from the font's colour glyphs.
    Color,
}

impl RasterStyle {
    /// Returns how many bytes one pixel of a bitmap of this style takes.
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Subpixel => 3,
            Self::Color => 4,
        }
    }
}
