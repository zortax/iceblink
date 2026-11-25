//! Rows of bins across scratch pages.

use iceblink::MAX_MARK_PAGES;

/// Places bins left to right in rows, row under row, page after page.
#[derive(Debug)]
pub(crate) struct Shelves {
    /// The size of a page.
    extent: [u32; 2],
    page: u32,
    /// The next free texel of the current row.
    cursor: [u32; 2],
    /// The height of the current row.
    row: u32,
}

impl Shelves {
    pub(crate) fn new(extent: [u32; 2]) -> Self {
        Self {
            extent,
            page: 0,
            cursor: [0, 0],
            row: 0,
        }
    }

    /// Returns the page and position of a bin of `size`, or `None` when every page is full.
    ///
    /// `size` fits within one page.
    pub(crate) fn place(&mut self, size: [u32; 2]) -> Option<(u32, [u32; 2])> {
        if self.cursor[0] + size[0] > self.extent[0] {
            self.cursor = [0, self.cursor[1] + self.row];
            self.row = 0;
        }
        if self.cursor[1] + size[1] > self.extent[1] {
            self.page += 1;
            self.cursor = [0, 0];
            self.row = 0;
        }
        if self.page >= MAX_MARK_PAGES {
            return None;
        }

        let at = self.cursor;
        self.cursor[0] += size[0];
        self.row = self.row.max(size[1]);
        Some((self.page, at))
    }
}
