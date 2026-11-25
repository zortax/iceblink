//! Single lines of text, laid out with the font's advances.

use iceblink::{
    ArenaRange, ChunkRecorder, ClipRef, FontData, FontId, Glyph, GlyphFlags, GlyphRun, PaintRef,
    SpaceRef,
};
use skrifa::MetadataProvider as _;
use skrifa::instance::{LocationRef, Size};

/// A font registered with the compositor, and its file.
pub struct Font {
    /// The id the compositor knows the font by.
    pub id: FontId,
    /// The font file.
    pub data: FontData,
}

/// A line of text set in one font at one size.
pub struct Line<'a> {
    /// The text.
    pub text: &'a str,
    /// The font size, in the chunk's units.
    pub size: f32,
    /// The start of the baseline.
    pub origin: [f32; 2],
    /// The paint of the glyphs.
    pub paint: PaintRef,
}

impl Font {
    /// Records `line` as one glyph run, with each glyph placed at the advance of the one before.
    ///
    /// A character the font has no glyph for is set as the font's missing glyph.
    ///
    /// # Panics
    ///
    /// Panics if the font file does not parse.
    pub fn record(&self, chunk: &mut ChunkRecorder<'_>, line: &Line<'_>) {
        let face = skrifa::FontRef::from_index(self.data.bytes(), self.data.index)
            .expect("the font parses");
        let charmap = face.charmap();
        let metrics = face.glyph_metrics(Size::new(line.size), LocationRef::default());
        let mut x = line.origin[0];
        let glyphs: Vec<Glyph> = line
            .text
            .chars()
            .map(|character| {
                let id = charmap.map(character).unwrap_or_default();
                let glyph = Glyph {
                    id: id.to_u32(),
                    x,
                    y: line.origin[1],
                };
                x += metrics.advance_width(id).unwrap_or(line.size / 2.0);
                glyph
            })
            .collect();
        let glyphs = chunk.glyphs(&glyphs);
        chunk.glyph_run(GlyphRun {
            font: self.id,
            size: line.size,
            glyphs,
            coords: ArenaRange::default(),
            paint: line.paint,
            embolden: 0.0,
            skew: 0.0,
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
            flags: GlyphFlags::default(),
            _pad: [0; 1],
            palette: 0,
        });
    }
}
