//! Glyph rasterisation.

use crate::kurbo::BezPath;
use crate::{FontData, GlyphBitmap, GlyphKey, GlyphPlacement, OutlineKey};

/// Rasterises glyphs into bitmaps and writes their outlines.
///
/// Equal keys and variation coordinates produce identical bytes. Every input that changes pixels
/// is in the key. Several threads may call any method concurrently with different keys.
///
/// `coords` are the font's normalised variation coordinates in F2Dot14, one per axis in
/// the font's axis order, which the key's `instance` stands for. An empty slice is the default
/// instance.
pub trait GlyphRaster: Send + Sync {
    /// Rasterises one glyph into `out` and returns where the bitmap sits.
    ///
    /// The result depends on the glyph:
    ///
    /// - If the glyph is blank, such as a space, `out` is zero-sized and the result is `Some`.
    /// - If the font has no such glyph, the result is `None`.
    /// - Otherwise `out` holds the bitmap in the layout of `key.style`, with `Subpixel` in the
    ///   stripe order `RedToBlue`.
    fn rasterize(
        &self,
        font: &FontData,
        key: &GlyphKey,
        coords: &[i16],
        out: &mut GlyphBitmap,
    ) -> Option<GlyphPlacement>;

    /// Rasterises several glyphs of one font at one set of coordinates, calling `done` with each
    /// key's index in `keys`, its result and its bitmap.
    ///
    /// Each result and bitmap equals what `rasterize` returns for the same key. The calls follow
    /// the order of `keys`. An implementation that serialises `rasterize` on shared state
    /// overrides this method to give each batch state of its own.
    fn rasterize_many(
        &self,
        font: &FontData,
        coords: &[i16],
        keys: &[GlyphKey],
        done: &mut dyn FnMut(usize, Option<GlyphPlacement>, &GlyphBitmap),
    ) {
        let mut bitmap = GlyphBitmap::default();
        for (index, key) in keys.iter().enumerate() {
            let placement = self.rasterize(font, key, coords, &mut bitmap);
            done(index, placement, &bitmap);
        }
    }

    /// Reports whether the font draws `glyph` from colour layers or a colour bitmap.
    ///
    /// The default reports no colour glyphs.
    fn is_color(&self, font: &FontData, glyph: u32) -> bool {
        let _ = (font, glyph);
        false
    }

    /// Writes one glyph's outline into `out`, or returns false when the font has no outline for
    /// it, such as a glyph it lacks or draws only from a bitmap.
    ///
    /// The curves are at the key's device size, in device pixels, with y down and the glyph's
    /// origin at zero. A blank glyph writes an empty path and returns true.
    fn outline(&self, font: &FontData, key: &OutlineKey, coords: &[i16], out: &mut BezPath)
    -> bool;
}
