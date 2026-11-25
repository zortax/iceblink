//! The keys atlas tiles are pinned and looked up by.

use iceblink_alloc::atlas::AtlasKey;

/// An atlas tile that a lowering drew from.
///
/// Every lowering appends the keys of the tiles its instances read. A pinned key keeps its tile,
/// and [`ContentCache::is_resident`](crate::content::ContentCache::is_resident) tells whether an
/// unpinned one survived an eviction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey(pub(crate) AtlasKey);

impl TileKey {
    /// Returns the atlas key.
    pub(crate) const fn atlas(self) -> AtlasKey {
        self.0
    }
}
