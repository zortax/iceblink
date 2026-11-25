//! Snapshots of atlas occupancy.

use crate::atlas::cache::Atlas;
use crate::atlas::texture::TextureKind;

/// A snapshot of what an [`Atlas`] holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasReport {
    /// The number of cached entries.
    pub tiles: usize,
    /// The number of entries with at least one hold.
    pub referenced_tiles: usize,
    /// The bytes of the held entries' own tiles, each in its pool's format.
    ///
    /// A held tile keeps its whole texture resident, so holds keep at least this much memory.
    pub referenced_bytes: u64,
    /// The number of textures across all pools.
    pub textures: usize,
    /// The texels of those textures.
    pub texels: u64,
    /// The bytes of those textures, weighted by each pool's format.
    pub bytes: u64,
    /// The number of queued uploads.
    pub pending_uploads: usize,
    /// The bytes of the queued uploads.
    pub pending_bytes: u64,
}

impl Atlas {
    /// Returns a snapshot of what the atlas holds.
    pub fn report(&self) -> AtlasReport {
        let mut textures = 0;
        let mut texels = 0;
        let mut bytes = 0;
        for kind in TextureKind::ALL {
            let pool = &self.pools[kind.index()];
            textures += pool.live_textures();
            texels += pool.texels();
            bytes += pool.texels() * u64::from(kind.format().bytes_per_texel());
        }
        AtlasReport {
            tiles: self.index.len(),
            referenced_tiles: self.held_tiles,
            referenced_bytes: self.held_bytes,
            textures,
            texels,
            bytes,
            pending_uploads: self.pending.len(),
            pending_bytes: self.pending_bytes,
        }
    }
}
