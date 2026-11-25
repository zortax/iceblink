//! Bringing a renderer that holds no page up to date.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{DeviceRect, PageOp};

use crate::content::cache::ContentCache;
use crate::content::image::ImageStore;
use crate::content::image::levels::levels_below;
use crate::content::image::place::level_zero;

impl ContentCache {
    /// Appends the page operations that recreate every page, as the drained page operations
    /// left it, for a renderer that holds no page.
    ///
    /// Every page is created, then its texels are uploaded: image tiles from `images`, and the
    /// other pages from copies the cache keeps of them, so nothing is rasterised again. The
    /// cache's page operations must all be drained first, which
    /// [`ContentCache::has_page_ops`] reports.
    pub fn replay_pages(&self, images: &ImageStore, out: &mut Vec<PageOp>) {
        debug_assert!(
            !self.has_page_ops(),
            "page operations were replayed before they were drained"
        );

        self.tiles.replay(out);
        for stored in images.iter() {
            let Some(placed) = self.images.get(stored.serial) else {
                continue;
            };
            let Some(tile) = self.tiles.peek(placed.key) else {
                continue;
            };

            let top = level_zero(stored, placed.factor);
            let [x, y, width, height] = tile.rect;
            let below = if placed.levels {
                levels_below(top.as_slice(), [width, height])
            } else {
                Vec::new()
            };

            out.push(upload(tile.page, 0, [x, y, width, height], top.as_slice()));
            for (index, texels) in below.iter().enumerate() {
                let level = index as u32 + 1;
                let [width, height] = PageOp::level_size([width, height], level);
                out.push(upload(tile.page, level, [0, 0, width, height], texels));
            }
        }
    }

    /// Returns the bytes of the page copies the cache keeps for [`ContentCache::replay_pages`].
    pub fn page_copy_bytes(&self) -> u64 {
        self.tiles.copied_bytes()
    }
}

fn upload(page: u32, level: u32, rect: [u32; 4], texels: &[u8]) -> PageOp {
    let [x, y, width, height] = rect;
    let rect: DeviceRect = Rect::new(
        Point::new(x as i32, y as i32),
        Size::new(width as i32, height as i32),
    );
    PageOp::Upload {
        page,
        level,
        rect,
        bytes: Arc::from(texels),
        offset: 0,
        stride: width * 4,
    }
}
