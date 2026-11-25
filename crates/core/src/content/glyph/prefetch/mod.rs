//! Rasterising the new glyphs of many runs at once, on worker threads.

mod batch;
mod workers;

use iceblink::{FontData, FontId, GlyphKey, GlyphRaster, RasterStyle};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::content::cache::ContentCache;
use crate::content::glyph::keys::RunKeys;
use crate::content::glyph::memo::Drawn;
use crate::content::glyph::route::Route;
use crate::content::input::RunJob;

/// The fewest new glyphs rasterised on worker threads.
// One glyph costs about 15 µs to hint and rasterise. Below this count the serial loop finishes
// before the workers start and prepare their hinting state.
const MIN_PARALLEL: usize = 128;

impl ContentCache {
    /// Rasterises the glyphs of `jobs` that have no tile, and places them in the atlas.
    ///
    /// The glyphs are the ones [`ContentCache::lower_glyph_run`] would rasterise for the same
    /// runs, each once. They are sorted by font, variation coordinates, size and style, and cut
    /// into batches of one font and one set of coordinates. With at least 128 glyphs the batches
    /// run on scoped worker threads, one per available core, and otherwise on the calling thread.
    /// On `wasm32` they always run on the calling thread. The bitmaps are then placed in sorted
    /// order, so the atlas holds the same tiles at the same places however the batches ran.
    ///
    /// The keys of the runs on the atlas route are kept until the next prefetch, and lowering one
    /// of those runs with the same glyphs under the same keys reuses them.
    ///
    /// Returns the number of glyphs rasterised.
    pub fn prefetch_glyph_runs(&mut self, jobs: &[RunJob<'_>]) -> usize {
        self.glyphs.memo.clear();
        let Some(raster) = self.raster.clone() else {
            return 0;
        };

        let mut fonts: FxHashMap<FontId, Option<FontData>> = FxHashMap::default();
        let mut misses = self.glyph_misses(&*raster, jobs, &mut fonts);
        if misses.is_empty() {
            return 0;
        }

        misses.sort_unstable_by_key(|key| {
            (
                key.font.0,
                key.instance,
                key.size_bits,
                key.embolden_bits,
                key.skew_bits,
                style_rank(key.style),
                key.hint,
                key.palette,
                key.phase,
                key.glyph,
            )
        });
        let count = misses.len();
        let parallel = count >= MIN_PARALLEL;
        let batches = batch::cut(misses, &fonts, &self.glyphs.instances, parallel);
        let ran = workers::rasterize(&*raster, &batches, parallel);
        self.tally.prefetch_batches += ran.on_workers as u64;
        self.tally.glyphs_rasterised += count as u64;
        self.tally.glyphs_prefetched += count as u64;

        for (batch, answers) in batches.into_iter().zip(ran.answers) {
            for (key, answer) in batch.keys.into_iter().zip(answers) {
                self.place_answer(key, answer);
            }
        }
        count
    }

    /// Returns the keys of the glyphs of `jobs` that need rasterising, each once, and remembers
    /// the keys of the runs on the atlas route.
    ///
    /// The font of every run is added to `fonts`, as `None` for a font the registry lacks.
    fn glyph_misses(
        &mut self,
        raster: &dyn GlyphRaster,
        jobs: &[RunJob<'_>],
        fonts: &mut FxHashMap<FontId, Option<FontData>>,
    ) -> Vec<GlyphKey> {
        let mut seen: FxHashSet<GlyphKey> = FxHashSet::default();
        let mut misses: Vec<GlyphKey> = Vec::new();
        for job in jobs {
            let id = job.input.run.font;
            let font = match fonts.get(&id) {
                Some(font) => font.clone(),
                None => {
                    let font = self.font(id);
                    fonts.insert(id, font.clone());
                    font
                }
            };
            let Some(font) = font else {
                continue;
            };
            let instance = self.glyphs.instances.intern(job.input.coords);
            let keys = RunKeys::of(&job.input, &job.target, instance);
            // A run on the outline route lowers its glyphs from outlines first.
            let remembers = keys.route != Route::Outline;
            let start = self.glyphs.memo.start();
            for glyph in job.input.glyphs {
                let entry = if self.glyphs.colors.is_color(raster, id, &font, glyph.id) {
                    if keys.silhouette {
                        Drawn::Nothing
                    } else {
                        let (key, spot) = keys.color(glyph);
                        Drawn::Color(key, spot)
                    }
                } else if keys.route == Route::Outline {
                    continue;
                } else {
                    let (key, spot) = keys.plain(glyph);
                    Drawn::Plain(key, spot)
                };
                if remembers {
                    self.glyphs.memo.push(*glyph, entry);
                }
                let (Drawn::Color(key, _) | Drawn::Plain(key, _)) = entry else {
                    continue;
                };
                if self.glyphs.answers.needs_raster(&self.tiles, &key) && seen.insert(key) {
                    misses.push(key);
                }
            }
            if remembers {
                self.glyphs.memo.finish(id, keys, start);
            }
        }
        misses
    }
}

/// A sort rank for a raster style, which has no order of its own.
fn style_rank(style: RasterStyle) -> u8 {
    match style {
        RasterStyle::Mono => 0,
        RasterStyle::Subpixel => 1,
        RasterStyle::Color => 2,
    }
}
