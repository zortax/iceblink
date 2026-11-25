//! The tiles images are drawn from.

use iceblink_alloc::atlas::AtlasKey;
use rustc_hash::FxHashMap;

use crate::content::key::TileKey;

/// The tile one image is placed in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ImageTile {
    pub(crate) key: TileKey,
    /// How many image texels one tile texel covers along each axis.
    pub(crate) factor: u32,
    /// Whether the tile is a page of its own, with levels of detail.
    pub(crate) levels: bool,
}

/// The tile of every placed image, by the image's serial number.
#[derive(Debug, Default)]
pub(crate) struct ImageTiles {
    tiles: FxHashMap<u64, ImageTile>,
    serials: FxHashMap<AtlasKey, u64>,
}

impl ImageTiles {
    /// Returns the tile of the image with `serial`.
    pub(crate) fn get(&self, serial: u64) -> Option<ImageTile> {
        self.tiles.get(&serial).copied()
    }

    /// Records the tile of the image with `serial`.
    pub(crate) fn insert(&mut self, serial: u64, tile: ImageTile) {
        if let Some(previous) = self.tiles.insert(serial, tile) {
            self.serials.remove(&previous.key.atlas());
        }
        self.serials.insert(tile.key.atlas(), serial);
    }

    /// Forgets the image tiles that were removed.
    pub(crate) fn forget_tiles(&mut self, removed: &[AtlasKey]) {
        for key in removed {
            if let Some(serial) = self.serials.remove(key) {
                self.tiles.remove(&serial);
            }
        }
    }

    /// Forgets every tile.
    pub(crate) fn clear(&mut self) {
        self.tiles.clear();
        self.serials.clear();
    }
}
