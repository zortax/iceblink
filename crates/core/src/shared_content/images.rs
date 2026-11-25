//! Client images in the image store, and the tiles mask clips read.

use iceblink::{AtlasTile, ClientId, ImageDesc, ImageWrite};

use crate::content::ImageKey;
use crate::shared_content::Content;

/// Returns the store key of a client's image slot.
pub(crate) fn image_key(client: ClientId, slot: u32) -> ImageKey {
    ImageKey((u64::from(client.0) << 32) | u64::from(slot))
}

impl Content {
    /// Creates the pixels of a new image, all transparent.
    pub(crate) fn create_image(&mut self, key: ImageKey, desc: ImageDesc) {
        self.release_image(key);
        // The commit validated the description.
        let _ = self.images.create(key, desc);
    }

    /// Writes texels into an image and uploads them into its tile, if it has one.
    pub(crate) fn write_image(&mut self, key: ImageKey, write: &ImageWrite) {
        if self.images.write(key, write).is_ok() {
            self.cache.refresh_image(&self.images, key, write);
        }
    }

    /// Removes an image's pixels and its tile.
    pub(crate) fn release_image(&mut self, key: ImageKey) {
        if self.images.desc(key).is_none() {
            return;
        }

        if self.masks.remove(&key)
            && let Ok(tile) = self.cache.image_tile(&self.images, key)
        {
            self.cache.unpin(&[tile.key]);
        }
        self.cache.release_image(&self.images, key);
        self.images.remove(key);
    }

    /// Returns the tile a mask clip reads the `source` texels of an image from, pinned for as
    /// long as the image exists.
    ///
    /// An image that has no texels, or that no page had room for, has an empty tile.
    pub(crate) fn mask_tile(&mut self, key: ImageKey, source: [f32; 4]) -> AtlasTile {
        let Ok(placed) = self.cache.image_tile(&self.images, key) else {
            return AtlasTile::default();
        };
        if self.masks.insert(key) {
            self.cache.pin(&[placed.key]);
        }

        let [x, y, width, height] = placed.tile.rect;
        let texel =
            |value: f32, limit: u32| ((value * placed.scale).round().max(0.0) as u32).min(limit);
        let x0 = texel(source[0], width);
        let y0 = texel(source[1], height);
        let x1 = texel(source[0] + source[2], width).max(x0);
        let y1 = texel(source[1] + source[3], height).max(y0);
        AtlasTile {
            page: placed.tile.page,
            rect: [x + x0, y + y0, x1 - x0, y1 - y0],
        }
    }
}
