//! Checking each kind of item.

use iceblink::FontRegistry;
use iceblink::{
    Backdrop, ExternalQuad, GlyphRun, Image, Line, Marks, PathItem, Quad, ShaderMode, ShaderQuad,
    Shadow,
};

use super::{Check, range};
use crate::commit::finite;
use crate::scene::kind::Kind;

impl Check<'_> {
    /// Checks every entry of every item array.
    pub(super) fn items(&mut self, fonts: &dyn FontRegistry) {
        let chunk = self.chunk;
        chunk.quads.iter().for_each(|item| self.quad(item));
        chunk
            .shader_quads
            .iter()
            .for_each(|item| self.shader_quad(item));
        chunk.shadows.iter().for_each(|item| self.shadow(item));
        chunk.lines.iter().for_each(|item| self.line(item));
        chunk
            .glyph_runs
            .iter()
            .for_each(|item| self.glyph_run(item, fonts));
        chunk.images.iter().for_each(|item| self.image(item));
        chunk.marks.iter().for_each(|item| self.mark(item));
        chunk.paths.iter().for_each(|item| self.path(item));
        chunk.externals.iter().for_each(|item| self.external(item));
        chunk.backdrops.iter().for_each(|item| self.backdrop(item));
    }

    fn quad(&mut self, item: &Quad) {
        self.finite(item.bounds.iter().chain(&item.radii).chain(&item.shapes));
        self.finite(item.border.iter().chain(&item.paint_origin));
        self.valid(item.style.is_valid());
        self.paint(item.fill);
        self.paint(item.stroke);
        self.clip(item.clip);
        self.space(item.space);
    }

    fn shader_quad(&mut self, item: &ShaderQuad) {
        self.finite(item.bounds.iter().chain(&item.radii).chain(&item.shapes));
        self.finite(item.border.iter().chain(&item.paint_origin));
        self.finite([&item.opacity]);
        self.paint(item.fill);
        self.paint(item.stroke);
        self.shader(item.shader, &[ShaderMode::PAINT, ShaderMode::COVERAGE]);
        self.slot(Kind::Params, item.params);
        self.clip(item.clip);
        self.space(item.space);
    }

    fn shadow(&mut self, item: &Shadow) {
        self.finite(
            item.shape_bounds
                .iter()
                .chain(&item.radii)
                .chain(&item.caster_bounds),
        );
        self.finite(item.caster_radii.iter().chain(&item.shapes));
        self.finite([&item.blur]);
        self.color(&item.color);
        self.valid(item.inset <= 1);
        self.clip(item.clip);
        self.space(item.space);
    }

    fn line(&mut self, item: &Line) {
        self.finite(item.bounds.iter().chain([&item.thickness]));
        self.color(&item.color);
        self.valid(item.style.is_valid());
        self.clip(item.clip);
        self.space(item.space);
    }

    fn glyph_run(&mut self, item: &GlyphRun, fonts: &dyn FontRegistry) {
        let chunk = self.chunk;
        self.finite([&item.size, &item.embolden, &item.skew]);
        self.valid(item.flags.is_valid());
        self.valid(fonts.get(item.font).is_some());
        match range(item.glyphs, chunk.glyphs.len()) {
            Some(glyphs) => {
                let finite = chunk.glyphs[glyphs]
                    .iter()
                    .all(|glyph| glyph.x.is_finite() && glyph.y.is_finite());
                self.valid(finite);
            }
            None => self.valid(false),
        }
        self.valid(range(item.coords, chunk.coords.len()).is_some());
        self.paint(item.paint);
        self.clip(item.clip);
        self.space(item.space);
    }

    fn image(&mut self, item: &Image) {
        self.finite(item.bounds.iter().chain(&item.frame).chain(&item.source));
        self.finite(item.radii.iter().chain(&item.shapes).chain([&item.opacity]));
        self.valid(item.flags.is_valid() && item.filter.is_valid());
        self.valid(item.extend.iter().all(|extend| extend.is_valid()));
        self.paint(item.paint);
        self.slot(Kind::Image, item.image);
        self.clip(item.clip);
        self.space(item.space);
    }

    fn mark(&mut self, item: &Marks) {
        self.finite(
            item.bounds
                .iter()
                .chain(&item.paint_origin)
                .chain(&item.origin),
        );
        self.finite(item.axes.iter().chain([&item.half_width]));
        self.valid(item.flags.is_valid());
        self.marks(item);
        self.paint(item.paint);
        self.clip(item.clip);
        self.space(item.space);
    }

    /// Checks a path item's paints, fill rules, geometry and slots.
    fn path(&mut self, item: &PathItem) {
        if let Some(fill) = &item.fill {
            self.valid(fill.rule.is_valid());
            self.paint(fill.paint);
        }
        if let Some(stroke) = &item.stroke {
            self.valid(finite::stroke(&stroke.style));
            self.paint(stroke.paint);
        }
        for clip in item.clips.iter() {
            self.valid(clip.rule.is_valid() && finite::path(&clip.path));
        }

        let affine = item
            .placement
            .as_coeffs()
            .into_iter()
            .chain(item.brush.as_coeffs());
        self.valid(affine.into_iter().all(f64::is_finite));
        self.valid(finite::path(&item.path));

        self.clip(item.clip);
        self.space(item.space);
    }

    fn external(&mut self, item: &ExternalQuad) {
        self.finite(item.bounds.iter().chain([&item.opacity]));
        self.slot(Kind::Texture, item.texture);
        self.clip(item.clip);
        self.space(item.space);
    }

    fn backdrop(&mut self, item: &Backdrop) {
        self.finite(item.bounds.iter());
        self.valid((item.filters as usize) < self.chunk.filters.len());
        self.clip(item.clip);
        self.space(item.space);
    }
}
