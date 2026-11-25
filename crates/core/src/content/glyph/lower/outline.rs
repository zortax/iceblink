//! Glyphs on the outline route, lowered into paths.

use std::sync::Arc;

use iceblink::{FillRule, Glyph, OutlineKey, PathFill, PathStroke};
use kurbo::{Affine, Stroke};

use super::Run;
use crate::content::cache::ContentCache;
use crate::content::glyph::outlines::Outline;
use crate::content::lowering::{Lowered, Lowering};
use crate::content::path::{PlacedPath, frame_path};

impl ContentCache {
    /// Appends `glyph` of `run` as a path filled with the run's paint, and returns false when the
    /// font has no outline for it.
    ///
    /// A blank outline appends nothing.
    pub(super) fn outline_glyph(
        &mut self,
        run: &Run<'_>,
        glyph: &Glyph,
        out: &mut Lowering,
    ) -> bool {
        let Run {
            raster,
            font,
            input,
            keys,
            target,
        } = *run;

        let base = keys.base();
        let key = OutlineKey {
            font: base.font,
            glyph: glyph.id,
            size_bits: base.size_bits,
            instance: base.instance,
            skew_bits: base.skew_bits,
        };
        let path = match self
            .glyphs
            .outlines
            .outline(raster, font, &key, input.coords)
        {
            Outline::Missing => return false,
            Outline::Curves(path) => path,
        };
        if path.elements().is_empty() {
            return true;
        }

        let paint = input.run.paint;
        // The stroke is drawn in path space, which an affine world matrix scales on the way to
        // the device. A projected path is stroked on the device.
        let scale = target.world.to_affine2().map_or(1.0, |affine| {
            f64::from((affine.a * affine.d - affine.b * affine.c).abs()).sqrt()
        });
        // The stroke straddles the outline, so twice the emboldening grows each side by it.
        let width = 2.0 * f64::from(input.run.embolden) / scale;
        let [x, y] = keys.origin(glyph);
        let placed = PlacedPath {
            path: &path,
            placement: Affine::translate((f64::from(x), f64::from(y))),
            // The paint is read in local space, from the paint origin the chunk offset moves.
            brush: Affine::translate((-f64::from(glyph.x), -f64::from(glyph.y))),
            fill: Some(PathFill {
                paint,
                rule: FillRule::NON_ZERO,
            }),
            stroke: (width > 0.0 && width.is_finite()).then(|| PathStroke {
                paint,
                style: Stroke::new(width),
            }),
            clips: &Arc::clone(&self.glyphs.no_clips),
        };
        out.items
            .push(Lowered::Path(frame_path(placed, &target.world)));
        self.tally.glyphs_outlined += 1;
        true
    }
}
