//! The test font, and glyph runs laid out in it on one line.

use iceblink::{
    ArenaRange, ChunkRecorder, ClipRef, FontData, FontId, Glyph, GlyphFlags, GlyphRun, PaintRef,
    SpaceRef,
};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};

/// Reads the test font, Noto Sans.
pub(crate) fn test_font() -> FontData {
    iceblink_testing::fonts::read_font(iceblink_testing::fonts::LATIN)
}

/// Returns the glyphs of `text` set in the test font at `size`, from a pen at `origin` on the
/// baseline.
pub(crate) fn line(text: &str, size: f32, origin: [f32; 2]) -> Vec<Glyph> {
    let font = test_font();
    let face = skrifa::FontRef::new(font.bytes()).expect("the test font parses");
    let charmap = face.charmap();
    let metrics = face.glyph_metrics(Size::new(size), LocationRef::default());
    let mut x = origin[0];
    text.chars()
        .map(|character| {
            let id = charmap.map(character).unwrap_or_default();
            let glyph = Glyph {
                id: id.to_u32(),
                x,
                y: origin[1],
            };
            x += metrics.advance_width(id).unwrap_or(size / 2.0);
            glyph
        })
        .collect()
}

/// Records a glyph run of `text` at `size` with its baseline starting at `origin`.
pub(crate) fn run(
    chunk: &mut ChunkRecorder<'_>,
    font: FontId,
    text: &str,
    size: f32,
    origin: [f32; 2],
    paint: PaintRef,
) {
    run_with(
        chunk,
        font,
        text,
        size,
        origin,
        paint,
        GlyphFlags::default(),
    );
}

/// Records a glyph run as [`run`] does, rasterised with `flags`.
pub(crate) fn run_with(
    chunk: &mut ChunkRecorder<'_>,
    font: FontId,
    text: &str,
    size: f32,
    origin: [f32; 2],
    paint: PaintRef,
    flags: GlyphFlags,
) {
    let glyphs = chunk.glyphs(&line(text, size, origin));
    chunk.glyph_run(GlyphRun {
        font,
        size,
        glyphs,
        coords: ArenaRange::default(),
        paint,
        embolden: 0.0,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags,
        _pad: [0; 1],
        palette: 0,
    });
}
