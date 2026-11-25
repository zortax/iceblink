//! Images placed in atlas tiles, and their tiles released.

use std::sync::Arc;

use iceblink::AtlasTile;
use iceblink_alloc::atlas::{AtlasError, TextureKind, UploadBytes};

use crate::content::cache::ContentCache;
use crate::content::error::ContentError;
use crate::content::image::downsample::{downsample, factor_for, reduced};
use crate::content::image::levels::levels_below;
use crate::content::image::store::{ImageKey, ImageStore, StoredImage};
use crate::content::image::tiles::ImageTile;
use crate::content::key::TileKey;

/// An image's tile, for drawing it or masking with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageTileRef {
    /// The whole image on its page.
    pub tile: AtlasTile,
    /// The key to pin the tile by.
    pub key: TileKey,
    /// Tile texels per image texel, below 1 for an image reduced to fit a page.
    pub scale: f32,
}

impl ContentCache {
    /// Returns the tile of the image of `image`, placing the image first if it has none.
    ///
    /// An image wider or taller than half a shared page gets a page of its own, with levels of
    /// detail down to one texel. An image larger than the largest page is reduced by a whole
    /// factor to fit, averaging each block of texels.
    ///
    /// # Errors
    ///
    /// - [`ContentError::UnknownImage`] if `images` holds no image of `image`.
    /// - [`ContentError::EmptyImage`] if the image has no texels.
    /// - [`ContentError::Atlas`] if no tile was found after evicting the oldest unpinned tiles
    ///   once.
    pub fn image_tile(
        &mut self,
        images: &ImageStore,
        image: ImageKey,
    ) -> Result<ImageTileRef, ContentError> {
        let stored = images.get(image).ok_or(ContentError::UnknownImage)?;
        if stored.desc.width == 0 || stored.desc.height == 0 {
            return Err(ContentError::EmptyImage);
        }

        if let Some(placed) = self.images.get(stored.serial)
            && let Some(tile) = self.tiles.get(placed.key)
        {
            return Ok(tile_ref(tile, placed));
        }

        let placed = match self.place_image(stored) {
            Err(AtlasError::OutOfSpace { .. }) if self.evict_oldest() => self.place_image(stored),
            placed => placed,
        }?;
        self.images.insert(stored.serial, placed.1);
        self.tally.image_tiles_placed += 1;
        Ok(tile_ref(placed.0, placed.1))
    }

    /// Removes the tile of the image of `image` unless it is pinned or used in the current
    /// frame.
    ///
    /// The image must still be in `images`, since its tile is found through it. A tile that stays
    /// is evicted like any other.
    pub fn release_image(&mut self, images: &ImageStore, image: ImageKey) {
        let Some(stored) = images.get(image) else {
            return;
        };
        let Some(placed) = self.images.get(stored.serial) else {
            return;
        };
        if self.tiles.remove_if_unused(placed.key) {
            self.forget(&[placed.key.atlas()]);
        }
    }

    /// Places `stored` in a new tile.
    fn place_image(&mut self, stored: &StoredImage) -> Result<(AtlasTile, ImageTile), AtlasError> {
        let size = [stored.desc.width, stored.desc.height];
        let factor = factor_for(size, self.config.max_page_size);
        let extent = reduced(size, factor);
        let key = self.tiles.fresh_key(TextureKind::Image);
        let build = || level_zero(stored, factor);
        let shared = self.config.shared_image_side();
        let own = extent[0] > shared || extent[1] > shared;

        let tile = if own {
            self.tiles.insert_own_page(key, extent, || {
                let top = build();
                let below = levels_below(top.as_slice(), extent);
                std::iter::once(top)
                    .chain(below.into_iter().map(UploadBytes::Owned))
                    .collect()
            })?
        } else {
            self.tiles.insert(key, extent, &mut Some(build()))?
        };

        Ok((
            tile,
            ImageTile {
                key,
                factor,
                levels: own,
            },
        ))
    }
}

/// Returns the texels of `stored` reduced by `factor`, as its tile holds them.
pub(crate) fn level_zero(stored: &StoredImage, factor: u32) -> UploadBytes {
    if factor > 1 {
        let size = [stored.desc.width, stored.desc.height];
        UploadBytes::Owned(downsample(&stored.texels, size, factor))
    } else {
        UploadBytes::Shared(Arc::clone(&stored.texels))
    }
}

/// Returns the public tile of a placed image.
fn tile_ref(tile: AtlasTile, placed: ImageTile) -> ImageTileRef {
    ImageTileRef {
        tile,
        key: placed.key,
        scale: 1.0 / placed.factor as f32,
    }
}
