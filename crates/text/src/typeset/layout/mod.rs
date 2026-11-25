//! Text broken into lines, as glyph runs and decoration lines, with selection and caret geometry.

mod cursor;
mod decoration;
mod flatten;
mod item;

use std::sync::Arc;

use iceblink::Glyph;
use parley::{Alignment, AlignmentOptions};

use crate::typeset::registry::FontRegistry;
use crate::typeset::style::Ink;

pub use self::cursor::Position;
pub use self::item::{TextItem, TextRun};

use self::item::Piece;

/// A shaped text broken into lines.
///
/// Coordinates are in layout units, from the top left of the layout box, with y growing down.
pub struct Layout {
    inner: parley::Layout<Ink>,
    registry: Arc<FontRegistry>,
    glyphs: Vec<Glyph>,
    coords: Vec<i16>,
    pieces: Vec<Piece>,
}

impl Layout {
    pub(crate) fn new(
        inner: parley::Layout<Ink>,
        registry: Arc<FontRegistry>,
        width: Option<f32>,
        alignment: Alignment,
    ) -> Self {
        let mut layout = Self {
            inner,
            registry,
            glyphs: Vec::new(),
            coords: Vec::new(),
            pieces: Vec::new(),
        };
        layout.break_lines(width, alignment);
        layout
    }

    /// Breaks the text into lines again, without shaping it again.
    ///
    /// Lines break at `width` if given, and are aligned within it, or within the widest line
    /// without one.
    pub fn break_lines(&mut self, width: Option<f32>, alignment: Alignment) {
        self.inner.break_all_lines(width);
        self.inner.align(alignment, AlignmentOptions::default());
        flatten::flatten(self);
    }

    /// Returns the width and height of the laid out text.
    ///
    /// The width is that of the widest line, without trailing whitespace.
    pub fn size(&self) -> [f32; 2] {
        [self.inner.width(), self.inner.height()]
    }

    /// Returns the glyph runs and decoration lines in paint order.
    ///
    /// The lines under and over a run come before it, and the line through it after it.
    pub fn items(&self) -> impl Iterator<Item = TextItem<'_>> {
        self.pieces.iter().map(|piece| piece.item(self))
    }
}
