//! Removing tiles: one at a time, by eviction, or all at once.

use iceblink_alloc::atlas::AtlasKey;

use crate::content::key::TileKey;
use crate::content::tiles::TileStore;

impl TileStore {
    /// Removes the tile of `key` if it is unpinned and unused in the current generation, and
    /// returns whether it was removed.
    pub(crate) fn remove_if_unused(&mut self, key: TileKey) -> bool {
        if self.atlas.used_this_frame(key.atlas())
            || !self.atlas.remove_if_unreferenced(key.atlas())
        {
            return false;
        }
        self.rects.remove(&key.atlas());
        self.evicted += 1;
        true
    }

    /// Removes the unpinned tiles of the oldest generation before the current one, and appends
    /// their keys to `removed`.
    ///
    /// Returns whether anything was removed.
    pub(crate) fn evict_oldest(&mut self, removed: &mut Vec<AtlasKey>) -> bool {
        let start = removed.len();
        let step = self.atlas.evict_least_recently_used_into(removed);
        self.forget_rects(&removed[start..]);
        !step.is_empty()
    }

    /// Removes unpinned tiles, oldest generation first, until the pages hold at most `limit`
    /// bytes or no unpinned tile is left, and appends their keys to `removed`.
    ///
    /// Tiles used in the current generation are kept. Returns the bytes of the pages destroyed.
    pub(crate) fn evict_to(&mut self, limit: u64, removed: &mut Vec<AtlasKey>) -> u64 {
        let before = self.bytes();
        while self.bytes() > limit {
            if !self.evict_oldest(removed) {
                break;
            }
        }
        before - self.bytes()
    }

    /// Removes every tile, pinned ones included, and destroys every page.
    ///
    /// Returns the bytes of the pages destroyed.
    pub(crate) fn clear(&mut self) -> u64 {
        let before = self.bytes();
        self.evicted += self.atlas.len() as u64;
        self.atlas.clear();
        self.rects.clear();
        before
    }

    /// Drops the rectangles of removed tiles and counts them as evicted.
    fn forget_rects(&mut self, removed: &[AtlasKey]) {
        self.evicted += removed.len() as u64;
        for key in removed {
            self.rects.remove(key);
        }
    }
}
