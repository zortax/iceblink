//! Ranges of a chunk's arenas.

use core::ops::Range;

use bytemuck::{Pod, Zeroable};

/// A range of one of a chunk's arenas, such as its glyphs or its variation coordinates.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct ArenaRange {
    /// The index of the first entry.
    pub start: u32,
    /// The number of entries.
    pub len: u32,
}

impl ArenaRange {
    /// Returns the indices of the range, for slicing the arena.
    pub const fn indices(self) -> Range<usize> {
        let start = self.start as usize;
        // Saturates on 32-bit targets, so an unvalidated range fails to slice instead of wrapping.
        start..start.saturating_add(self.len as usize)
    }
}
