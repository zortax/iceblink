//! Glyph runs lowered into sprites and paths.

mod outline;
mod sprite;

use iceblink::{FontData, GlyphKey, GlyphRaster, RasterStyle};

use crate::content::cache::ContentCache;
use crate::content::glyph::keys::{RunKeys, Spot};
use crate::content::glyph::memo::Drawn;
use crate::content::glyph::route::Route;
use crate::content::input::{GlyphRunInput, RunJob};
use crate::content::lowering::{Lowered, Lowering};
use crate::content::target::Target;

impl ContentCache {
    /// Lowers a glyph run drawn at `target`, appending its instances and tiles to `out`.
    ///
    /// Each glyph becomes one instance, in glyph order:
    ///
    /// - A colour glyph becomes a colour sprite that takes its alpha from the run's paint, or
    ///   nothing when the run has `SILHOUETTE`.
    /// - On the atlas route, another glyph becomes a subpixel sprite when the input allows it and
    ///   the run asks for it, and a mono sprite otherwise.
    /// - On the outline route, another glyph becomes a path filled with the run's paint, and
    ///   stroked so each side grows by `embolden` device pixels when that is above zero. A glyph
    ///   without an outline becomes a mono sprite resampled through the world matrix.
    ///
    /// Glyphs seen for the first time are rasterised one at a time.
    /// [`ContentCache::lower_glyph_runs`] rasterises them in parallel first. A blank glyph, a
    /// glyph the font lacks and a glyph the atlas has no room for produce nothing. Without a
    /// rasteriser or a font registry, or for a font the registry lacks, the run produces nothing.
    pub fn lower_glyph_run(
        &mut self,
        input: &GlyphRunInput<'_>,
        target: &Target,
        out: &mut Lowering,
    ) {
        let Some(raster) = self.raster.clone() else {
            return;
        };
        let Some(font) = self.font(input.run.font) else {
            return;
        };

        let instance = self.glyphs.instances.intern(input.coords);
        let keys = RunKeys::of(input, target, instance);
        // Most glyphs become one instance that reads one tile.
        out.items.reserve(input.glyphs.len());
        out.tiles.reserve(input.glyphs.len());
        let run = Run {
            raster: &*raster,
            font: &font,
            input,
            keys: &keys,
            target,
        };

        if keys.route != Route::Outline
            && let Some(found) = self.glyphs.memo.find(input.run.font, &keys, input.glyphs)
        {
            for position in found {
                match self.glyphs.memo.drawn(position) {
                    Drawn::Nothing => {}
                    Drawn::Color(key, spot) => self.color_sprite(&run, &key, spot, out),
                    Drawn::Plain(key, spot) => self.plain_sprite(&run, &key, spot, out),
                }
            }
            return;
        }

        for glyph in input.glyphs {
            let color = self
                .glyphs
                .colors
                .is_color(&*raster, input.run.font, &font, glyph.id);

            if color {
                if !keys.silhouette {
                    let (key, spot) = keys.color(glyph);
                    self.color_sprite(&run, &key, spot, out);
                }
                continue;
            }

            if keys.route == Route::Outline && self.outline_glyph(&run, glyph, out) {
                continue;
            }

            let (key, spot) = keys.plain(glyph);
            self.plain_sprite(&run, &key, spot, out);
        }
    }

    /// Appends the colour sprite of the glyph with `key` at `spot`, if it has a tile.
    fn color_sprite(&mut self, run: &Run<'_>, key: &GlyphKey, spot: Spot, out: &mut Lowering) {
        let Some(placed) = self.glyph_tile(run.raster, run.font, run.input.coords, key) else {
            return;
        };

        let sprite = sprite::color_glyph(run.input.run.paint, run.target, spot, &placed);
        out.items.push(Lowered::Color(sprite));
        out.tiles.push(placed.key);
        self.tally.glyphs_placed += 1;
    }

    /// Appends the coverage sprite of the glyph with `key` at `spot`, if it has a tile.
    fn plain_sprite(&mut self, run: &Run<'_>, key: &GlyphKey, spot: Spot, out: &mut Lowering) {
        let Some(placed) = self.glyph_tile(run.raster, run.font, run.input.coords, key) else {
            return;
        };

        let sprite = sprite::coverage_glyph(run.input.run.paint, run.target, spot, &placed);
        out.items.push(match key.style {
            RasterStyle::Subpixel => Lowered::Subpixel(sprite),
            _ => Lowered::Mono(sprite),
        });
        out.tiles.push(placed.key);
        self.tally.glyphs_placed += 1;
    }

    /// Lowers several glyph runs, appending their instances and tiles to `out` in job order.
    ///
    /// Glyphs without a tile are rasterised first, in parallel, as
    /// [`ContentCache::prefetch_glyph_runs`] does. The result equals lowering each run in turn.
    pub fn lower_glyph_runs(&mut self, jobs: &[RunJob<'_>], out: &mut Lowering) {
        self.prefetch_glyph_runs(jobs);
        for job in jobs {
            self.lower_glyph_run(&job.input, &job.target, out);
        }
    }
}

/// One run being lowered, with its rasteriser, font, keys and target.
#[derive(Clone, Copy)]
struct Run<'a> {
    raster: &'a dyn GlyphRaster,
    font: &'a FontData,
    input: &'a GlyphRunInput<'a>,
    keys: &'a RunKeys,
    target: &'a Target,
}
