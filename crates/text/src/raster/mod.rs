//! Glyph rasterisation into bitmaps.

mod bitmap;
mod bold;
mod render;
mod scale;
mod stripes;

use std::sync::Mutex;

use iceblink::GlyphRaster;
use iceblink::kurbo::BezPath;
use iceblink::{FontData, GlyphBitmap, GlyphKey, GlyphPlacement, OutlineKey};

use crate::outline::Outlines;
use crate::raster::scale::Scratch;

/// A glyph rasteriser over swash for bitmaps and skrifa for outlines.
///
/// Bitmaps are hinted when the key asks for it, rasterised at the key's phase, and drawn from the
/// face's colour layers or colour strikes for colour keys.
///
/// A mono or subpixel bitmap with emboldening is the outline filled and stroked twice the
/// emboldening wide, so every side grows by the emboldening. Colour keys are not emboldened.
///
/// Concurrent `rasterize` calls take turns on one scale context. Each `rasterize_many` batch takes
/// a context of its own, so batches on several threads run in parallel. Contexts are reused by
/// later batches, which find their faces' hinting state prepared.
#[derive(Default)]
pub struct SwashRaster {
    scratch: Mutex<Scratch>,
    batches: Mutex<Vec<Scratch>>,
    outlines: Outlines,
}

impl SwashRaster {
    /// Returns a rasteriser with empty caches.
    pub fn new() -> Self {
        Self::default()
    }
}

impl core::fmt::Debug for SwashRaster {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SwashRaster")
            .finish_non_exhaustive()
    }
}

impl GlyphRaster for SwashRaster {
    fn rasterize(
        &self,
        font: &FontData,
        key: &GlyphKey,
        coords: &[i16],
        out: &mut GlyphBitmap,
    ) -> Option<GlyphPlacement> {
        let mut scratch = self
            .scratch
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        scratch.rasterize(font, key, coords, out)
    }

    fn rasterize_many(
        &self,
        font: &FontData,
        coords: &[i16],
        keys: &[GlyphKey],
        done: &mut dyn FnMut(usize, Option<GlyphPlacement>, &GlyphBitmap),
    ) {
        // A batch sorted by size prepares each face's hinting state once, and a context that
        // prepared it before keeps it.
        let mut scratch = self
            .batches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop()
            .unwrap_or_default();

        let mut bitmap = GlyphBitmap::default();
        for (index, key) in keys.iter().enumerate() {
            let placement = scratch.rasterize(font, key, coords, &mut bitmap);
            done(index, placement, &bitmap);
        }

        self.batches
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(scratch);
    }

    fn is_color(&self, font: &FontData, glyph: u32) -> bool {
        crate::color::is_color(font, glyph)
    }

    fn outline(
        &self,
        font: &FontData,
        key: &OutlineKey,
        coords: &[i16],
        out: &mut BezPath,
    ) -> bool {
        self.outlines.write(font, key, coords, out)
    }
}
