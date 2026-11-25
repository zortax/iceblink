//! Eviction under a byte budget, trimming and forgetting.

use crate::content::cache::ContentCache;

impl ContentCache {
    /// Evicts unpinned tiles until the pages hold at most `to_bytes`, and returns the bytes of
    /// the pages destroyed.
    ///
    /// The least recently used tiles are evicted first. Pinned tiles and tiles used in the current
    /// frame stay, so the pages can remain above `to_bytes`. The destructions are recorded as page
    /// operations, and [`ContentCache::memory`] no longer counts the destroyed pages on return.
    pub fn trim(&mut self, to_bytes: u64) -> u64 {
        let mut removed = Vec::new();
        let freed = self.tiles.evict_to(to_bytes, &mut removed);

        self.forget(&removed);
        freed
    }

    /// Trims to `budget` when one is set and the pages exceed it, and returns the bytes of the
    /// pages destroyed.
    pub fn enforce_budget(&mut self, budget: Option<u64>) -> u64 {
        match budget {
            Some(limit) if self.tiles.bytes() > limit => self.trim(limit),
            _ => 0,
        }
    }

    /// Drops every tile, pinned ones included, and destroys every page.
    ///
    /// Glyph outlines and colour answers are dropped too. Images stay in their store and are
    /// placed again when next lowered. Instances lowered before the call read removed tiles and
    /// must be lowered again.
    ///
    /// Returns the bytes of the pages destroyed.
    pub fn forget_caches(&mut self) -> u64 {
        let freed = self.tiles.clear();
        self.glyphs.clear();
        self.masks.clear();
        self.marks.clear();
        self.images.clear();
        freed
    }

    /// Drops every tile and page as [`ContentCache::forget_caches`] does, without recording a
    /// destruction, for a renderer that lost its pages.
    ///
    /// Page operations not yet drained are dropped too, so the next drain creates every page it
    /// uploads into.
    ///
    /// Returns the bytes of the pages dropped.
    pub fn reset_device(&mut self) -> u64 {
        let freed = self.forget_caches();
        self.tiles.discard_ops();
        freed
    }
}
