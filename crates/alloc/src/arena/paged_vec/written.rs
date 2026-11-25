//! The record of pages written since the last compaction.

/// The pages written since the last compaction, each listed once.
// An unwritten page cannot have become droppable, so compaction costs the writes and not the table.
#[derive(Debug, Default)]
pub(super) struct Written {
    /// Whether each page is listed.
    flagged: Vec<bool>,
    pages: Vec<usize>,
}

impl Written {
    /// Creates an empty record.
    pub(super) const fn new() -> Self {
        Self {
            flagged: Vec::new(),
            pages: Vec::new(),
        }
    }

    /// Records a write to `page`.
    #[inline]
    pub(super) fn mark(&mut self, page: usize) {
        if page >= self.flagged.len() {
            self.flagged.resize(page + 1, false);
        }
        if !self.flagged[page] {
            self.flagged[page] = true;
            self.pages.push(page);
        }
    }

    /// Removes and returns every listed page.
    pub(super) fn drain(&mut self) -> Vec<usize> {
        let pages = core::mem::take(&mut self.pages);
        for &page in &pages {
            self.flagged[page] = false;
        }
        pages
    }
}
