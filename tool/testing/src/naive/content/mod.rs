//! The compositor's atlas: pages of glyph tiles, images, mark cells and path masks, and the caches
//! that place content on them.

mod glyphs;
mod images;
mod marks;
mod masks;
mod pages;
mod shelf;

use std::collections::{HashMap, HashSet};

use iceblink::geom::{Point, Size};
use iceblink::{DeviceRect, PageOp};

pub(crate) use self::glyphs::{Glyphs, Raster};
pub(crate) use self::images::{ImagePage, Images};
pub(crate) use self::marks::{MarkCells, Marks};
pub(crate) use self::masks::{
    Mask, MaskCommand, MaskStyle, MaskTiles, PathId, Routes, StrokeCap, StrokeJoin,
};
pub(crate) use self::pages::{PageUse, Pages, bytes_per_texel};
pub(crate) use self::shelf::Shelves;

use self::pages::Page;

/// The largest side of a shared page.
const SHARED_SIDE: u32 = 1024;

/// Every page and what is placed on them, and the histories that route paths onto masks.
#[derive(Default)]
pub(crate) struct Content {
    pub(crate) pages: Pages,
    pub(crate) glyphs: Glyphs,
    pub(crate) images: Images,
    pub(crate) marks: Marks,
    pub(crate) masks: MaskTiles,
    pub(crate) routes: Routes,
}

/// The pages a renderer holds, with the version of each it last received.
pub(crate) type HeldPages = HashMap<u32, u64>;

impl Content {
    /// Returns the side of a new shared page for a renderer whose textures reach `max_side`.
    pub(crate) fn shared_side(max_side: u32) -> u32 {
        SHARED_SIDE.min(max_side.max(1))
    }

    /// Adds to `ops` what brings a renderer holding `held` up to date, and updates `held`.
    ///
    /// Pages the renderer holds that no longer exist are destroyed. Pages it lacks are created,
    /// and every level of a page it lacks or holds an older version of is uploaded whole.
    pub(crate) fn sync(&mut self, held: &mut HeldPages, ops: &mut Vec<PageOp>) {
        held.retain(|page, _| {
            let exists = self.pages.pages.contains_key(page);
            if !exists {
                ops.push(PageOp::Destroy { page: *page });
            }
            exists
        });

        for (id, page) in &mut self.pages.pages {
            let known = held.get(id).copied();
            if known == Some(page.version) {
                continue;
            }

            if known.is_none() {
                ops.push(PageOp::Create {
                    page: *id,
                    format: page.format,
                    size: page.size,
                    levels: page.level_count(),
                });
            }

            upload(*id, page, ops);
            held.insert(*id, page.version);
        }
    }

    /// Records that the frame numbered `frame` drew from `pages`.
    pub(crate) fn drawn(&mut self, pages: &HashSet<u32>, frame: u64) {
        for id in pages {
            if let Some(page) = self.pages.pages.get_mut(id) {
                page.last_drawn = frame;
            }
        }
    }

    /// Destroys unpinned pages, least recently drawn first, until the pages hold at most
    /// `to_bytes`, and returns the bytes destroyed.
    pub(crate) fn evict(&mut self, to_bytes: u64, pinned: &HashSet<u32>) -> u64 {
        let mut candidates: Vec<(u64, u32)> = self
            .pages
            .pages
            .iter()
            .filter(|(id, _)| !pinned.contains(id))
            .map(|(id, page)| (page.last_drawn, *id))
            .collect();
        candidates.sort_unstable();

        let mut total = self.pages.bytes();
        let mut destroyed = 0;
        for (_, id) in candidates {
            if total <= to_bytes {
                break;
            }
            let bytes = self.destroy(id);
            total -= bytes;
            destroyed += bytes;
        }

        destroyed
    }

    /// Destroys every page, forgets every rasterised glyph and outline and every path's route
    /// history, and returns the bytes destroyed.
    pub(crate) fn forget(&mut self) -> u64 {
        let bytes = self.pages.bytes();
        let ids: Vec<u32> = self.pages.pages.keys().copied().collect();
        for id in ids {
            self.destroy(id);
        }
        self.glyphs = Glyphs::default();
        self.routes.forget();
        bytes
    }

    /// Destroys one page and the cache entries on it, and returns the page's bytes.
    fn destroy(&mut self, id: u32) -> u64 {
        let Some(page) = self.pages.pages.get(&id) else {
            return 0;
        };

        match page.purpose {
            PageUse::Glyphs => self.glyphs.evict(id),
            PageUse::Image => self.images.evict(id),
            PageUse::Marks => self.marks.evict(id),
            PageUse::Masks => self.masks.evict(id),
        }
        self.pages.destroy(id)
    }
}

/// Adds to `ops` an upload of every level of `page`, whole.
fn upload(id: u32, page: &mut Page, ops: &mut Vec<PageOp>) {
    for level in 0..page.level_count() {
        let [width, height] = PageOp::level_size(page.size, level);
        ops.push(PageOp::Upload {
            page: id,
            level,
            rect: DeviceRect::new(Point::new(0, 0), Size::new(width as i32, height as i32)),
            bytes: page.level(level),
            offset: 0,
            stride: width * bytes_per_texel(page.format) as u32,
        });
    }
}
