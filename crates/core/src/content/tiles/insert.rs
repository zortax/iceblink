//! Placing tiles.

use iceblink::AtlasTile;
use iceblink::geom::Size;
use iceblink_alloc::atlas::{AtlasError, UploadBytes};

use crate::content::key::TileKey;
use crate::content::tiles::TileStore;

impl TileStore {
    /// Places `bytes` as the tile of `key`, or returns the tile `key` already holds.
    ///
    /// The bytes are tightly packed rows of `size` texels in the format of the key's pool. They
    /// are taken out of `bytes` only when the tile is placed, so a refused insertion leaves them
    /// for a retry.
    ///
    /// # Errors
    ///
    /// Returns the atlas's refusal: the size exceeds the largest page, or the pool is full.
    pub(crate) fn insert(
        &mut self,
        key: TileKey,
        size: [u32; 2],
        bytes: &mut Option<UploadBytes>,
    ) -> Result<AtlasTile, AtlasError> {
        let tile = self.atlas.get_or_insert(key.atlas(), extent(size), || {
            bytes.take().unwrap_or(UploadBytes::Owned(Vec::new()))
        })?;
        Ok(self.remember(key, tile))
    }

    /// Places the levels `build` returns as the tile of `key` on a page of its own, or returns
    /// the tile `key` already holds.
    ///
    /// `build` returns level 0 followed by each level below it, as tightly packed rows. It is
    /// called before the page is allocated, so a refused insertion has consumed its bytes.
    ///
    /// # Errors
    ///
    /// Returns the atlas's refusal: the size exceeds the largest page, or the pool may not grow.
    pub(crate) fn insert_own_page(
        &mut self,
        key: TileKey,
        size: [u32; 2],
        build: impl FnOnce() -> Vec<UploadBytes>,
    ) -> Result<AtlasTile, AtlasError> {
        let tile = self
            .atlas
            .insert_standalone(key.atlas(), extent(size), build)?;
        Ok(self.remember(key, tile))
    }
}

/// Returns `size` as an atlas extent, saturating at `i32::MAX`.
fn extent(size: [u32; 2]) -> Size<i32, iceblink::geom::Device> {
    Size::new(
        i32::try_from(size[0]).unwrap_or(i32::MAX),
        i32::try_from(size[1]).unwrap_or(i32::MAX),
    )
}
