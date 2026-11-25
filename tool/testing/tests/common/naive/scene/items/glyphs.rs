//! The glyph runs of the scene.

use iceblink::{ChunkRecorder, GlyphFlags, PaintRef, SpaceRef};

use super::{Fonts, Refs};
use crate::common::naive::scene::{glyph_run, glyphs};

/// Records one line of glyphs as four runs: hinted subpixel text, gradient text in the turned
/// space, text in the fixed space, and colour glyphs.
pub(super) fn record(recorder: &mut ChunkRecorder<'_>, refs: &Refs, fonts: Fonts) {
    let range = recorder.glyphs(&glyphs());
    let black = PaintRef::color([0, 0, 0, 255]);
    recorder.glyph_run(glyph_run(
        fonts.latin,
        range,
        black,
        GlyphFlags::SUBPIXEL | GlyphFlags::HINT,
        SpaceRef::INHERIT,
    ));
    recorder.glyph_run(glyph_run(
        fonts.latin,
        range,
        refs.gradient,
        GlyphFlags::default(),
        refs.turned,
    ));
    recorder.glyph_run(glyph_run(
        fonts.latin,
        range,
        black,
        GlyphFlags::default(),
        refs.fixed,
    ));
    recorder.glyph_run(glyph_run(
        fonts.color,
        range,
        PaintRef::color([0, 0, 0, 200]),
        GlyphFlags::default(),
        SpaceRef::INHERIT,
    ));
}
