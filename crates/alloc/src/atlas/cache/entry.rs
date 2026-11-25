//! The bookkeeping of one cached entry.

use iceblink::geom::{Device, Size};

use crate::atlas::key::AtlasKey;
use crate::atlas::tile::AtlasTile;

/// One cached entry.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry {
    pub(crate) key: AtlasKey,
    pub(crate) tile: AtlasTile,
    /// The requested size, which is the size of the uploaded bytes.
    pub(crate) size: Size<i32, Device>,
    /// The hold count, saturating in both directions.
    pub(crate) refs: u32,
    /// The generation of the entry's last use.
    pub(crate) generation: u64,
}

impl Entry {
    /// Reports whether the entry has no holds.
    pub(crate) const fn is_unreferenced(&self) -> bool {
        self.refs == 0
    }

    /// Returns the bytes of the entry's tile in its pool's format.
    pub(crate) fn bytes(&self) -> u64 {
        self.tile.texture.format().bytes_for(
            self.size.width.max(0) as u32,
            self.size.height.max(0) as u32,
        )
    }

    /// Returns the texels of the entry's tile.
    pub(crate) fn texels(&self) -> u64 {
        self.size.width.max(0) as u64 * self.size.height.max(0) as u64
    }
}
