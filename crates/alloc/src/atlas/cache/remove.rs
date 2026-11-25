//! Removing entries.

use crate::atlas::cache::Atlas;
use crate::atlas::key::AtlasKey;
use crate::atlas::tile::AtlasTile;

impl Atlas {
    /// Removes `key` if it has no holds, and returns whether it was removed.
    pub fn remove_if_unreferenced(&mut self, key: AtlasKey) -> bool {
        let Some(&slot) = self.index.get(&key) else {
            return false;
        };
        if self.entries[slot as usize]
            .as_ref()
            .is_some_and(|entry| !entry.is_unreferenced())
        {
            return false;
        }
        self.remove(key)
    }

    /// Removes `key` and frees its tile, and returns whether `key` was cached.
    ///
    /// Holds do not prevent the removal. A queued upload for the tile is discarded, and a texture
    /// left without tiles is queued for destruction.
    pub fn remove(&mut self, key: AtlasKey) -> bool {
        let Some(slot) = self.index.remove(&key) else {
            return false;
        };
        let Some(entry) = self.entries[slot as usize].take() else {
            return false;
        };
        if !entry.is_unreferenced() {
            self.held_tiles -= 1;
            self.held_bytes -= entry.bytes();
        }
        self.free_slots.push(slot);
        self.used.forget(slot as usize);
        // The rectangle can be allocated again, and a stale upload would land in the new tile.
        self.discard_pending(entry.tile);
        self.pools[entry.tile.texture.kind.index()].deallocate(
            entry.tile.texture,
            entry.tile.tile,
            &mut self.textures,
        );
        true
    }

    /// Removes every entry and queues the destruction of every texture.
    ///
    /// Queued uploads are discarded. Tiles returned before the call refer to destroyed textures.
    pub fn clear(&mut self) {
        for pool in &mut self.pools {
            pool.clear(&mut self.textures);
        }
        self.entries.clear();
        self.free_slots.clear();
        self.index.clear();
        self.used.reset();
        self.held_tiles = 0;
        self.held_bytes = 0;
        self.pending.clear();
        self.pending_bytes = 0;
    }

    /// Drops every queued upload into `tile`.
    fn discard_pending(&mut self, tile: AtlasTile) {
        let mut discarded = 0;
        self.pending.retain(|upload| {
            let stale = upload.texture == tile.texture && upload.tile == tile.tile;
            if stale {
                discarded += upload.bytes.len() as u64;
            }
            !stale
        });
        self.pending_bytes -= discarded;
    }
}
