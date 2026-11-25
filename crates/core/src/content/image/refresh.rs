//! Image writes uploaded into the tiles of placed images.

use std::sync::Arc;

use iceblink::{ImageFormat, ImageWrite, PageOp};

use crate::content::cache::ContentCache;
use crate::content::image::downsample::{downsample, reduced};
use crate::content::image::levels::{changed_below, levels_below};
use crate::content::image::store::{ImageKey, ImageStore, written_rect};
use crate::content::key::TileKey;

impl ContentCache {
    /// Uploads the texels `write` replaced into the image's tile, after
    /// [`ImageStore::write`] accepted it.
    ///
    /// Premultiplied texels are uploaded from the write's own bytes. Other formats are uploaded
    /// from the store, and a reduced image uploads its whole tile again. The levels of detail of
    /// an image on a page of its own are computed again, and the texels the write changed are
    /// uploaded on each level. Nothing is uploaded for an image without a tile, which takes its
    /// texels from the store when it is placed.
    pub fn refresh_image(&mut self, images: &ImageStore, image: ImageKey, write: &ImageWrite) {
        let Some(stored) = images.get(image) else {
            return;
        };
        let Some(placed) = self.images.get(stored.serial) else {
            return;
        };
        if !self.tiles.contains(placed.key) {
            return;
        }

        let size = [stored.desc.width, stored.desc.height];
        if placed.factor > 1 {
            let extent = reduced(size, placed.factor);
            let texels = downsample(&stored.texels, size, placed.factor);
            let whole = [0, 0, extent[0], extent[1]];
            if placed.levels {
                let changed = changed_below(whole, extent);
                self.upload_levels(placed.key, levels_below(&texels, extent), extent, &changed);
            }
            self.tiles
                .upload(placed.key, 0, whole, texels.into(), 0, extent[0] * 4);
            return;
        }

        let Ok([x, y, width, height]) = written_rect(stored.desc, write) else {
            return;
        };
        let within = [x, y, width, height].map(|value| value as u32);
        if placed.levels {
            let changed = changed_below(within, size);
            self.upload_levels(
                placed.key,
                levels_below(&stored.texels, size),
                size,
                &changed,
            );
        }
        if stored.desc.format == ImageFormat::RGBA8_PREMULTIPLIED {
            self.tiles.upload(
                placed.key,
                0,
                within,
                Arc::clone(&write.bytes),
                write.offset,
                write.stride,
            );
            return;
        }

        let row = width * 4;
        let mut bytes = Vec::with_capacity(row * height);
        for line in y..y + height {
            let at = (line * stored.desc.width as usize + x) * 4;
            bytes.extend_from_slice(&stored.texels[at..at + row]);
        }
        self.tiles
            .upload(placed.key, 0, within, bytes.into(), 0, row as u32);
    }

    /// Uploads the rectangle `changed[i]` of level `i + 1` from `levels[i]`, for a tile whose
    /// level 0 is `size`.
    fn upload_levels(
        &mut self,
        key: TileKey,
        levels: Vec<Vec<u8>>,
        size: [u32; 2],
        changed: &[[u32; 4]],
    ) {
        for (index, (texels, rect)) in levels.into_iter().zip(changed).enumerate() {
            let level = index as u32 + 1;
            let [width, _] = PageOp::level_size(size, level);
            let offset = (rect[1] as usize * width as usize + rect[0] as usize) * 4;
            self.tiles
                .upload(key, level, *rect, texels.into(), offset, width * 4);
        }
    }
}
