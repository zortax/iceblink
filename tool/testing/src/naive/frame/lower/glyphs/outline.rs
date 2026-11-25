//! Glyphs on the path route, filled as paths.

use std::sync::Arc;

use iceblink::kurbo::{Affine, Stroke};
use iceblink::{
    FillRule, FramePath, Glyph, OutlineKey, PathClip, PathFill, PathStroke, RasterStyle,
};

use crate::naive::frame::builder::Builder;
use crate::naive::frame::item::{self, Placed};
use crate::naive::frame::lower::glyphs::{Run, origin, sprite};
use crate::naive::frame::lower::path::{self, Mapped, to_bounds};
use crate::naive::geometry::{Bounds, plane_affine};

/// Fills a glyph's outline as a path of its own, and returns its device ink.
///
/// The outline is taken at the run's size, slanted by its skew, and placed at the glyph origin
/// with the chunk offset. An emboldened run also strokes the outline `2 × embolden / √|det|` local
/// units wide, where `det` is the determinant of the world matrix. A glyph without an outline is
/// drawn from a mono tile.
pub(super) fn glyph(
    builder: &mut Builder<'_>,
    placed: &Placed,
    run: &Run<'_>,
    glyph: &Glyph,
) -> Bounds {
    let key = OutlineKey {
        font: run.run.font,
        glyph: glyph.id,
        size_bits: run.run.size.to_bits(),
        instance: run.instance,
        skew_bits: run.run.skew.to_bits(),
    };
    let raster = builder
        .env
        .raster
        .expect("glyph runs lower only with a rasteriser");
    let Some(outline) = builder
        .content
        .glyphs
        .outline(raster, &run.font, &key, run.coords)
    else {
        return sprite::tile(builder, placed, run, glyph, RasterStyle::Mono);
    };

    let stroke = (run.run.embolden > 0.0).then(|| {
        let width = 2.0 * f64::from(run.run.embolden) / device_scale(placed);
        PathStroke {
            paint: run.paint,
            style: Stroke::new(width),
        }
    });

    let [x, y] = origin(placed, glyph);
    let at = Affine::translate((f64::from(x), f64::from(y)));
    let reach = stroke
        .as_ref()
        .map_or(0.0, |stroke| path::reach(&stroke.style));
    let local = to_bounds(at.transform_rect_bbox(outline.control_box().inflate(reach, reach)));
    let device = local.mapped(&placed.world).unwrap_or(builder.env.output);
    let ink = placed.cut(device);
    if !item::shows(builder, ink) {
        return ink;
    }

    let no_clips: Arc<[PathClip]> = Arc::new([]);
    let Some(mapped) = Mapped::new(&placed.world, at, &outline, &no_clips) else {
        return ink;
    };

    let frame_path = FramePath {
        path: mapped.path,
        transform: mapped.transform,
        // Paint space is the run's local space before the chunk offset.
        brush: Affine::translate((-f64::from(glyph.x), -f64::from(glyph.y))),
        fill: Some(PathFill {
            paint: run.paint,
            rule: FillRule::NON_ZERO,
        }),
        stroke,
        clips: mapped.clips,
        ink: ink.round_out().expect("a shown ink is not empty"),
    };
    path::push(builder, placed, frame_path);
    ink
}

/// Returns how many device pixels one local unit spans, as the square root of the world matrix's
/// area scale.
fn device_scale(placed: &Placed) -> f64 {
    plane_affine(&placed.world)
        .map(|affine| affine.determinant().abs().sqrt())
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0)
}
