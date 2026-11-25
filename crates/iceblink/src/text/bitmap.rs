//! The pixels of one rasterised glyph.

use crate::text::RasterStyle;

/// The pixels of one rasterised glyph, row by row with no padding between rows.
///
/// A rasteriser writes every field. A blank glyph, such as a space, is zero pixels wide or high.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlyphBitmap {
    /// The width, in pixels.
    pub width: u32,
    /// The height, in pixels.
    pub height: u32,
    /// The pixels, in the layout of the key's `RasterStyle`.
    pub bytes: Vec<u8>,
}

impl GlyphBitmap {
    /// Reports whether the byte count matches the size for bitmaps of `style`.
    pub fn is_well_formed(&self, style: RasterStyle) -> bool {
        let pixels = self.width as usize * self.height as usize;
        self.bytes.len() == pixels * style.bytes_per_pixel()
    }

    /// Reports whether the bitmap has no pixels.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Where a glyph's bitmap sits: its top-left corner from the glyph's origin, x right and y up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphPlacement {
    /// The distance from the origin to the bitmap's left edge.
    pub left: i32,
    /// The distance from the origin up to the bitmap's top edge.
    pub top: i32,
}
