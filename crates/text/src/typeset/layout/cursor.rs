//! Selection rectangles, caret geometry and hit testing.

use std::ops::Range;

use iceblink::LocalRect;
use parley::{Affinity, BoundingBox, Cursor, Selection};

use crate::typeset::layout::Layout;

/// A position between two characters of a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    /// The byte offset of the character after the position.
    pub offset: usize,
    /// The line the position belongs to where a line wraps.
    ///
    /// - `Upstream` puts the position at the end of the earlier line.
    /// - `Downstream` puts it at the start of the later line.
    pub affinity: Affinity,
}

impl Position {
    /// Returns the position before the byte `offset`, on the line that holds it.
    pub const fn new(offset: usize) -> Self {
        Self {
            offset,
            affinity: Affinity::Downstream,
        }
    }
}

impl Layout {
    /// Returns the rectangles that cover the bytes in `range`, at most one per line.
    ///
    /// The rectangles span each line's height. A newline inside the range is covered by a short
    /// rectangle at the end of its line. An empty range has none.
    pub fn selection(&self, range: Range<usize>) -> Vec<LocalRect> {
        let start = Cursor::from_byte_index(&self.inner, range.start, Affinity::Downstream);
        let end = Cursor::from_byte_index(&self.inner, range.end, Affinity::Upstream);
        Selection::new(start, end)
            .geometry(&self.inner)
            .into_iter()
            .map(|(rect, _)| rect_of(rect))
            .collect()
    }

    /// Returns the rectangle of a caret `width` wide at `position`, as tall as its line.
    pub fn caret(&self, position: Position, width: f32) -> LocalRect {
        let cursor = Cursor::from_byte_index(&self.inner, position.offset, position.affinity);
        rect_of(cursor.geometry(&self.inner, width))
    }

    /// Returns the position nearest to `point`.
    ///
    /// For a point within a character, the position before or after it is returned, whichever is
    /// nearer. For a point above or below the text, a position on the first or last line is
    /// returned.
    pub fn hit_test(&self, point: [f32; 2]) -> Position {
        let cursor = Cursor::from_point(&self.inner, point[0], point[1]);
        Position {
            offset: cursor.index(),
            affinity: cursor.affinity(),
        }
    }
}

fn rect_of(rect: BoundingBox) -> LocalRect {
    [
        rect.x0 as f32,
        rect.y0 as f32,
        (rect.x1 - rect.x0) as f32,
        (rect.y1 - rect.y0) as f32,
    ]
}
