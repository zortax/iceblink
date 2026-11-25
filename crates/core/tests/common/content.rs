//! A fake glyph rasteriser, a font registry and item builders for the content tests.

use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use iceblink::kurbo::{BezPath, Rect, Shape};
use iceblink::{
    ArenaRange, ClipRef, FontData, FontId, Glyph, GlyphBitmap, GlyphFlags, GlyphKey,
    GlyphPlacement, GlyphRun, Matrix4, OutlineKey, PageOp, PaintKind, PaintRef, SpaceRef,
};
use iceblink::{FontRegistry, GlyphRaster};
use iceblink_core::content::{
    ContentCache, ContentConfig, GlyphRunInput, Lowered, Lowering, Target,
};

/// A glyph id the fake font lacks.
pub(crate) const MISSING: u32 = 9_999;
/// A glyph id that rasterises to no pixels.
pub(crate) const BLANK: u32 = 32;
/// A glyph id with a bitmap and no outline.
pub(crate) const NO_OUTLINE: u32 = 7_777;
/// The first colour glyph id.
pub(crate) const FIRST_COLOR: u32 = 1_000;

/// The bitmap's distance from the glyph origin to its left edge.
pub(crate) const LEFT: i32 = 1;
/// The bitmap's distance from the glyph origin up to its top edge.
pub(crate) const TOP: i32 = 5;

/// A rasteriser that draws a solid block per glyph and records every call.
#[derive(Default)]
pub(crate) struct FakeRaster {
    /// Every key rasterised, with the thread it ran on.
    pub(crate) calls: Mutex<Vec<(GlyphKey, ThreadId)>>,
    /// Every outline written.
    pub(crate) outlines: Mutex<Vec<OutlineKey>>,
    /// How long each batch sleeps before it rasterises.
    // Spreads batches over the workers.
    pub(crate) batch_delay: Duration,
}

impl FakeRaster {
    /// Returns a shared rasteriser.
    pub(crate) fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Returns the keys rasterised so far.
    pub(crate) fn keys(&self) -> Vec<GlyphKey> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|(key, _)| *key)
            .collect()
    }

    /// Returns the number of glyphs rasterised so far.
    pub(crate) fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    /// Returns the width a glyph of `key` is drawn with.
    pub(crate) fn width_of(key: &GlyphKey) -> u32 {
        4 + u32::from(key.phase)
    }
}

impl GlyphRaster for FakeRaster {
    fn rasterize(
        &self,
        _font: &FontData,
        key: &GlyphKey,
        _coords: &[i16],
        out: &mut GlyphBitmap,
    ) -> Option<GlyphPlacement> {
        self.calls
            .lock()
            .unwrap()
            .push((*key, std::thread::current().id()));

        if key.glyph == MISSING {
            return None;
        }
        if key.glyph == BLANK {
            out.width = 0;
            out.height = 0;
            out.bytes.clear();
            return Some(GlyphPlacement::default());
        }

        out.width = Self::width_of(key);
        out.height = 6;
        let value = (key.glyph % 200) as u8 + 1;
        let pixel: &[u8] = match key.style.bytes_per_pixel() {
            1 => &[value],
            3 => &[10, 20, 30],
            _ => &[200, 100, 50, 128],
        };
        out.bytes = pixel.repeat((out.width * out.height) as usize);
        Some(GlyphPlacement {
            left: LEFT,
            top: TOP,
        })
    }

    fn rasterize_many(
        &self,
        font: &FontData,
        coords: &[i16],
        keys: &[GlyphKey],
        done: &mut dyn FnMut(usize, Option<GlyphPlacement>, &GlyphBitmap),
    ) {
        std::thread::sleep(self.batch_delay);
        let mut bitmap = GlyphBitmap::default();
        for (index, key) in keys.iter().enumerate() {
            let placement = self.rasterize(font, key, coords, &mut bitmap);
            done(index, placement, &bitmap);
        }
    }

    fn outline(
        &self,
        _font: &FontData,
        key: &OutlineKey,
        _coords: &[i16],
        out: &mut BezPath,
    ) -> bool {
        self.outlines.lock().unwrap().push(*key);
        match key.glyph {
            MISSING | NO_OUTLINE => false,
            BLANK => true,
            _ => {
                let size = f64::from(key.size());
                *out = Rect::new(0.0, -size * 0.7, size * 0.5, 0.0).to_path(0.1);
                true
            }
        }
    }

    fn is_color(&self, _font: &FontData, glyph: u32) -> bool {
        glyph >= FIRST_COLOR && glyph != MISSING && glyph != NO_OUTLINE
    }
}

/// A registry that holds one font, `FontId(0)`.
pub(crate) struct OneFont;

impl FontRegistry for OneFont {
    fn register(&self, _data: FontData) -> FontId {
        FontId(0)
    }

    fn get(&self, id: FontId) -> Option<FontData> {
        (id == FontId(0)).then(|| FontData::new(b"fake font", 0))
    }

    fn generation(&self) -> u64 {
        1
    }
}

/// Returns a cache with `raster` and the one-font registry.
pub(crate) fn cache_with(raster: &Arc<FakeRaster>) -> ContentCache {
    cache_config(raster, ContentConfig::default())
}

/// Returns a cache with `raster`, the one-font registry and `config`.
pub(crate) fn cache_config(raster: &Arc<FakeRaster>, config: ContentConfig) -> ContentCache {
    let mut cache = ContentCache::new(config);
    cache.set_glyph_raster(Arc::clone(raster) as Arc<dyn GlyphRaster>);
    cache.set_font_registry(Arc::new(OneFont));
    cache.begin_frame();
    cache
}

/// Returns a solid paint reference to frame paint slot `index`.
pub(crate) fn solid(index: u32) -> PaintRef {
    PaintRef {
        kind: PaintKind::SOLID,
        _pad: [0; 3],
        index,
    }
}

/// Returns a gradient paint reference to frame paint slot `index`.
pub(crate) fn gradient(index: u32) -> PaintRef {
    PaintRef {
        kind: PaintKind::GRADIENT,
        ..solid(index)
    }
}

/// Returns a run of font zero at `size` with `flags`, painted with `paint`.
pub(crate) fn run(size: f32, paint: PaintRef, flags: GlyphFlags) -> GlyphRun {
    GlyphRun {
        font: FontId(0),
        size,
        glyphs: ArenaRange::default(),
        coords: ArenaRange::default(),
        paint,
        embolden: 0.0,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags,
        _pad: [0],
        palette: 0,
    }
}

/// Returns glyph `id` at `x`, `y`.
pub(crate) fn glyph(id: u32, x: f32, y: f32) -> Glyph {
    Glyph { id, x, y }
}

/// Returns the input of `run` with `glyphs`, at output scale 1 and without subpixel coverage.
pub(crate) fn input<'a>(run: &'a GlyphRun, glyphs: &'a [Glyph]) -> GlyphRunInput<'a> {
    GlyphRunInput {
        run,
        glyphs,
        coords: &[],
        subpixel: false,
        output_scale: 1.0,
    }
}

/// Returns a target under the translation `x`, `y`.
pub(crate) fn moved(x: f32, y: f32) -> Target {
    Target::new(Matrix4::translation(x, y, 0.0))
}

/// Lowers `input` under `target` and returns the lowering.
pub(crate) fn lower(
    cache: &mut ContentCache,
    input: &GlyphRunInput<'_>,
    target: &Target,
) -> Lowering {
    let mut out = Lowering::default();
    cache.lower_glyph_run(input, target, &mut out);
    out
}

/// Returns the bounds of every sprite of a lowering, in order.
pub(crate) fn sprite_bounds(lowering: &Lowering) -> Vec<[f32; 4]> {
    lowering
        .items
        .iter()
        .filter_map(|item| match item {
            Lowered::Mono(sprite) | Lowered::Subpixel(sprite) => Some(sprite.bounds),
            Lowered::Color(sprite) => Some(sprite.bounds),
            Lowered::Path(_) => None,
        })
        .collect()
}

/// Drains the page operations of `cache` into a vector.
pub(crate) fn drain(cache: &mut ContentCache) -> Vec<PageOp> {
    let mut ops = Vec::new();
    cache.drain_page_ops(&mut ops);
    ops
}

/// Returns the bytes of an upload operation, from its offset on.
pub(crate) fn upload_bytes(op: &PageOp) -> Option<&[u8]> {
    match op {
        PageOp::Upload { bytes, offset, .. } => Some(&bytes[*offset..]),
        _ => None,
    }
}
