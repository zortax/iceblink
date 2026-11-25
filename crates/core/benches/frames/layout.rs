//! Laying out lines of text in Noto Sans.

use iceblink::{FontData, Glyph};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};

/// Lays out lines of text in Noto Sans.
pub(crate) struct Layout {
    font: FontData,
}

impl Layout {
    pub(crate) fn new() -> Self {
        Self {
            font: iceblink_testing::fonts::read_font(iceblink_testing::fonts::LATIN),
        }
    }

    /// Returns the glyphs of `text` at `size`, from a pen at `origin` on the baseline.
    pub(crate) fn line(&self, text: &str, size: f32, origin: [f32; 2]) -> Vec<Glyph> {
        let face = skrifa::FontRef::new(self.font.bytes()).expect("the font parses");
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
}
