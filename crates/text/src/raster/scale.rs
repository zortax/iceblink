//! Rasterising one glyph through a scale context.

use iceblink::{FontData, GlyphBitmap, GlyphKey, GlyphPlacement, RasterStyle};
use swash::FontRef;
use swash::scale::ScaleContext;
use swash::scale::image::Image;
use swash::scale::outline::Outline;

use crate::raster::bitmap::{blank, repack};
use crate::raster::bold::{self, Layers};
use crate::raster::render::{phase, render, slant, sources};
use crate::raster::stripes;

/// The scale context and buffers one rasterisation needs.
#[derive(Default)]
pub(super) struct Scratch {
    context: ScaleContext,
    image: Image,
    outline: Outline,
    /// The four bytes per pixel a subpixel mask is rendered into.
    mask: Vec<u8>,
    layers: Layers,
}

impl Scratch {
    /// Rasterises `key` of `font` at `coords` into `out`.
    ///
    /// Returns `None` and blanks `out` when the font cannot be read or has no such glyph.
    pub(super) fn rasterize(
        &mut self,
        font: &FontData,
        key: &GlyphKey,
        coords: &[i16],
        out: &mut GlyphBitmap,
    ) -> Option<GlyphPlacement> {
        let Some(face) = FontRef::from_index(font.bytes(), font.index as usize) else {
            blank(out);
            return None;
        };
        let Ok(glyph) = u16::try_from(key.glyph) else {
            blank(out);
            return None;
        };

        let mut scaler = self
            .context
            .builder_with_id(face, identity(key, font.index))
            .size(key.size())
            .hint(key.hint)
            .normalized_coords(coords)
            .build();

        if drawn_from_outline(key)
            && scaler.has_outlines()
            && scaler.scale_outline_into(glyph, &mut self.outline)
        {
            slant(key, &mut self.outline);
            let placement = if key.embolden() > 0.0 {
                bold::rasterize(&self.outline, key, &mut self.layers, out)
            } else {
                stripes::rasterize(&self.outline, phase(key), &mut self.mask, out)
            };
            return Some(placement);
        }

        let sources = sources(key);
        self.image.clear();
        if !render(key, &sources).render_into(&mut scaler, glyph, &mut self.image) {
            // A glyph the font has but nothing draws, such as a space without an outline.
            blank(out);
            let exists = glyph < face.metrics(&[]).glyph_count;
            return exists.then_some(GlyphPlacement::default());
        }

        let placement = repack(&mut self.image, key.style, out);
        if out.is_empty() {
            blank(out);
        }
        Some(placement)
    }
}

/// Reports whether a bitmap of `key` is rasterised here from the glyph's outline.
///
/// Emboldened mono bitmaps and every subpixel bitmap are. Other bitmaps are rendered by swash.
// swash sizes a subpixel mask for the middle stripe, which cuts the outer ones, and its
// emboldening grows a glyph right and up only.
fn drawn_from_outline(key: &GlyphKey) -> bool {
    match key.style {
        RasterStyle::Mono => key.embolden() > 0.0,
        RasterStyle::Subpixel => true,
        RasterStyle::Color => false,
    }
}

/// Returns the id the scale context caches a face's prepared state under.
///
/// The id is stable across calls and distinct between faces and variation instances.
// A face's hinting programs then run once per instance.
fn identity(key: &GlyphKey, index: u32) -> [u64; 2] {
    [
        u64::from(key.font.0),
        u64::from(index) | (u64::from(key.instance) << 32),
    ]
}
