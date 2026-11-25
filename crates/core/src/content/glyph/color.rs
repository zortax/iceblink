//! Which glyphs are colour glyphs.

use iceblink::{FontData, FontId, GlyphRaster};
use rustc_hash::FxHashMap;

/// The rasteriser's colour answer for every glyph asked about so far.
#[derive(Debug, Default)]
pub(crate) struct ColorGlyphs {
    glyphs: FxHashMap<(FontId, u32), bool>,
}

impl ColorGlyphs {
    /// Reports whether `glyph` of `font` is a colour glyph, asking `raster` once per glyph.
    pub(crate) fn is_color(
        &mut self,
        raster: &dyn GlyphRaster,
        id: FontId,
        font: &FontData,
        glyph: u32,
    ) -> bool {
        *self
            .glyphs
            .entry((id, glyph))
            .or_insert_with(|| raster.is_color(font, glyph))
    }

    /// Forgets every answer.
    pub(crate) fn clear(&mut self) {
        self.glyphs.clear();
    }
}
