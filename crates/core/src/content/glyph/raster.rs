//! Rasterising glyphs and placing them, one at a time.

use iceblink::{FontData, GlyphBitmap, GlyphKey, GlyphPlacement, GlyphRaster};

use crate::content::cache::ContentCache;
use crate::content::glyph::answers::{Lookup, Placed, RasterAnswer};
use crate::content::glyph::texels::texels;

/// Returns the answer of one rasteriser call for `key`.
///
/// A glyph the font lacks, a blank glyph and a malformed bitmap all answer with no texels.
pub(crate) fn answer_of(
    key: &GlyphKey,
    placement: Option<GlyphPlacement>,
    bitmap: &GlyphBitmap,
) -> RasterAnswer {
    let style = key.style;
    match placement {
        Some(placement) if !bitmap.is_empty() && bitmap.is_well_formed(style) => RasterAnswer {
            placement,
            size: [bitmap.width, bitmap.height],
            texels: Some(texels(bitmap, style)),
        },
        Some(placement) => RasterAnswer {
            placement,
            size: [0, 0],
            texels: None,
        },
        None => RasterAnswer {
            placement: GlyphPlacement::default(),
            size: [0, 0],
            texels: None,
        },
    }
}

impl ContentCache {
    /// Returns the tile of `key`, rasterising and placing it on a miss.
    ///
    /// Returns `None` for a glyph with no pixels, and for one the atlas has no room for even after
    /// evicting the oldest unpinned tiles once.
    pub(crate) fn glyph_tile(
        &mut self,
        raster: &dyn GlyphRaster,
        font: &FontData,
        coords: &[i16],
        key: &GlyphKey,
    ) -> Option<Placed> {
        match self.glyphs.answers.lookup(&mut self.tiles, key) {
            Lookup::Hit(placed) => Some(placed),
            Lookup::Blank => None,
            Lookup::Miss => {
                let placement = raster.rasterize(font, key, coords, &mut self.bitmap);
                self.tally.glyphs_rasterised += 1;
                let answer = answer_of(key, placement, &self.bitmap);
                self.place_answer(*key, answer)
            }
        }
    }

    /// Places and remembers a rasterised glyph, evicting the oldest unpinned tiles once when the
    /// pool is full.
    pub(crate) fn place_answer(&mut self, key: GlyphKey, answer: RasterAnswer) -> Option<Placed> {
        match self.glyphs.answers.remember(&mut self.tiles, key, answer) {
            Ok(placed) => placed,
            Err(answer) => {
                // One eviction step spares every pinned tile and everything this frame used, so
                // one retry is safe.
                if !self.evict_oldest() {
                    return None;
                }
                self.glyphs
                    .answers
                    .remember(&mut self.tiles, key, answer)
                    .ok()
                    .flatten()
            }
        }
    }
}
