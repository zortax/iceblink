//! Glyph runs: the route of each glyph, and the sprites or paths it is drawn with.

mod outline;
mod sprite;

use iceblink::{FontData, Glyph, GlyphFlags, GlyphRun, Matrix4, PaintKind, PaintRef, RasterStyle};

use crate::naive::frame::builder::Builder;
use crate::naive::frame::item::{self, Placed};
use crate::naive::geometry::{Bounds, plane_affine};
use crate::naive::scene::Chunk;

/// A glyph run with everything its glyphs are lowered with.
pub(super) struct Run<'r> {
    pub(super) run: &'r GlyphRun,
    pub(super) font: FontData,
    pub(super) coords: &'r [i16],
    pub(super) instance: u32,
    /// The run's paint as a frame reference.
    pub(super) paint: PaintRef,
    /// The scale of the world matrix, when it is a uniform positive scale and a translation.
    pub(super) scale: Option<f32>,
}

/// Lowers a glyph run and returns its device ink, the union of what its glyphs draw.
///
/// Without a glyph rasteriser the run draws nothing and is counted.
pub(crate) fn run(builder: &mut Builder<'_>, chunk: &Chunk, run: &GlyphRun) -> Bounds {
    let Some(raster) = builder.env.raster else {
        builder.counters.glyph_runs_without_raster += 1;
        return Bounds::EMPTY;
    };

    let Some(font) = builder.env.fonts.get(run.font) else {
        return Bounds::EMPTY;
    };

    let Some(placed) = item::place(builder, chunk, run.space, run.clip) else {
        return Bounds::EMPTY;
    };

    let recorded = &chunk.recorded;
    let glyphs = recorded.glyphs.get(run.glyphs.indices()).unwrap_or(&[]);
    let coords = recorded.coords.get(run.coords.indices()).unwrap_or(&[]);
    let instance = builder.content.glyphs.instance(run.font, coords);
    let lowered = Run {
        run,
        font,
        coords,
        instance,
        paint: builder.paint(run.paint),
        scale: uniform_scale(&placed.world),
    };

    let atlas = atlas_route(builder, &lowered);
    let mut ink = Bounds::EMPTY;
    for glyph in glyphs {
        let glyph_ink = if raster.is_color(&lowered.font, glyph.id) {
            if run.flags.contains(GlyphFlags::SILHOUETTE) {
                continue;
            }
            sprite::tile(builder, &placed, &lowered, glyph, RasterStyle::Color)
        } else if atlas {
            let style = if subpixel(builder, &lowered) {
                RasterStyle::Subpixel
            } else {
                RasterStyle::Mono
            };
            sprite::tile(builder, &placed, &lowered, glyph, style)
        } else {
            outline::glyph(builder, &placed, &lowered, glyph)
        };
        ink = ink.union(glyph_ink);
    }

    ink
}

/// Reports whether a run takes the atlas route: a solid paint, a uniform positive scale, and a
/// device size no larger than the route threshold.
fn atlas_route(builder: &Builder<'_>, run: &Run<'_>) -> bool {
    let Some(scale) = run.scale else {
        return false;
    };

    let threshold = (96.0 * builder.env.config.scale.max(1.0)).min(256.0);
    is_solid(run.run.paint) && run.run.size * scale <= threshold
}

/// Reports whether a run's glyphs are drawn as subpixel sprites.
fn subpixel(builder: &Builder<'_>, run: &Run<'_>) -> bool {
    let flags = run.run.flags;
    builder.env.caps.subpixel_text
        && builder.env.subpixel.is_some()
        && builder.env.config.opaque
        && flags.contains(GlyphFlags::SUBPIXEL)
        && !flags.contains(GlyphFlags::SILHOUETTE)
        && is_solid(run.run.paint)
        && builder.groups_open == 0
}

fn is_solid(paint: PaintRef) -> bool {
    paint.kind == PaintKind::SOLID || paint.kind == PaintKind::INLINE
}

/// The largest relative difference of two scale factors that still count as one.
const UNIFORM: f64 = 1.0e-6;

/// Returns `s` when `world` maps `(x, y)` to `(a x + tx, d y + ty)` with `a` and `d` above zero
/// and equal within [`UNIFORM`], and `s` is the larger of them.
fn uniform_scale(world: &Matrix4) -> Option<f32> {
    let affine = plane_affine(world)?;
    let [a, b, c, d, _, _] = affine.as_coeffs();
    let upright = b == 0.0 && c == 0.0 && a > 0.0 && d > 0.0;
    (upright && (a - d).abs() <= UNIFORM * a.max(d)).then_some(a.max(d) as f32)
}

/// Returns the position of a glyph's origin in local space, with the chunk offset.
fn origin(placed: &Placed, glyph: &Glyph) -> [f32; 2] {
    [glyph.x + placed.offset[0], glyph.y + placed.offset[1]]
}
