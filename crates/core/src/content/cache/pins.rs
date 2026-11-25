//! Pinning tiles and asking whether they are still there.

use crate::content::cache::ContentCache;
use crate::content::key::TileKey;

impl ContentCache {
    /// Pins every tile of `keys` against eviction, once per entry.
    ///
    /// Pins count, so each pin is undone by one [`ContentCache::unpin`]. Keys without a tile are
    /// ignored.
    pub fn pin(&mut self, keys: &[TileKey]) {
        for &key in keys {
            self.tiles.pin(key);
        }
    }

    /// Releases one pin of every tile of `keys`, once per entry.
    pub fn unpin(&mut self, keys: &[TileKey]) {
        for &key in keys {
            self.tiles.unpin(key);
        }
    }

    /// Marks every tile of `keys` used in the current frame, which keeps it until the next
    /// [`ContentCache::begin_frame`] and makes it the most recently used.
    pub fn touch(&mut self, keys: &[TileKey]) {
        for &key in keys {
            self.tiles.get(key);
        }
    }

    /// Reports whether `key` still holds a tile.
    pub fn is_resident(&self, key: TileKey) -> bool {
        self.tiles.contains(key)
    }

    /// Reports whether every key of `keys` still holds a tile.
    pub fn all_resident(&self, keys: &[TileKey]) -> bool {
        keys.iter().all(|&key| self.tiles.contains(key))
    }

    /// Returns the number of tiles removed so far, by eviction, trimming, releasing an image,
    /// dropping replaced path masks or forgetting the caches.
    ///
    /// Instances lowered while the number did not change read only resident tiles.
    pub fn evictions(&self) -> u64 {
        self.tiles.evicted()
    }
}
