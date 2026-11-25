//! The local ink of a glyph run.

use iceblink::{Glyph, GlyphRun, LocalRect};

use crate::geometry::bounds::DeviceBox;

/// Returns a local ink that holds every glyph a run can draw, or `None` for a run without glyphs.
///
/// The box of the glyph origins is grown by twice the em size on every side, with the
/// emboldening on top.
pub(crate) fn glyph_run_ink(run: &GlyphRun, glyphs: &[Glyph]) -> Option<LocalRect> {
    // Stands in for the measured ink of a run whose glyphs cannot be rasterised or measured.
    let first = glyphs.first()?;
    let mut ink = DeviceBox {
        x0: first.x,
        y0: first.y,
        x1: first.x,
        y1: first.y,
    };
    for glyph in glyphs {
        ink.x0 = ink.x0.min(glyph.x);
        ink.y0 = ink.y0.min(glyph.y);
        ink.x1 = ink.x1.max(glyph.x);
        ink.y1 = ink.y1.max(glyph.y);
    }

    let reach = 2.0 * run.size.abs() + run.embolden.abs();
    Some(ink.outset([reach; 4]).rect())
}
