//! The content every output draws from, and the page operations that keep each renderer's pages
//! up to date.

mod images;
pub(crate) mod masks;
mod opaque;
pub(crate) mod pages;
pub(crate) mod prefetch;

use std::sync::Arc;

use iceblink::{FontRegistry, GlyphRaster, RenderCapabilities, ResourceWork};
use rustc_hash::FxHashSet;

use crate::content::{ContentCache, ContentConfig, ImageKey, ImageStore, TileKey};
use crate::output::renderer::RendererState;

use self::pages::{PageCursor, PageLog};

pub(crate) use self::images::image_key;

/// The content cache, the image pixels and the page operations not yet sent to every renderer.
///
/// One cache serves every output, so a chunk is lowered once for all of them.
pub(crate) struct Content {
    pub(crate) cache: ContentCache,
    pub(crate) images: ImageStore,
    pub(crate) pages: PageLog,
    /// The generation of the cache's tiles, which every reset of the cache advances.
    pub(crate) epoch: u64,
    /// A count that grows with every commit that may move lowered sprites off their pixels or
    /// phase.
    pub(crate) placements: u64,
    /// Device pixels per logical pixel of the output that draws the committing client.
    pub(crate) scale: f32,
    /// The images whose tiles mask clips read, which stay pinned while they exist.
    pub(crate) masks: FxHashSet<ImageKey>,
    raster: Option<Arc<dyn GlyphRaster>>,
    fonts: Option<Arc<dyn FontRegistry>>,
}

impl core::fmt::Debug for Content {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Content")
            .field("cache", &self.cache)
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

impl Content {
    pub(crate) fn new(fonts: Arc<dyn FontRegistry>) -> Self {
        let mut cache = ContentCache::new(ContentConfig::default());
        cache.set_font_registry(Arc::clone(&fonts));
        Self {
            cache,
            images: ImageStore::new(),
            pages: PageLog::default(),
            epoch: 0,
            placements: 0,
            scale: 1.0,
            masks: FxHashSet::default(),
            raster: None,
            fonts: Some(fonts),
        }
    }

    pub(crate) fn set_raster(&mut self, raster: Arc<dyn GlyphRaster>) {
        self.cache.set_glyph_raster(Arc::clone(&raster));
        self.raster = Some(raster);
    }

    pub(crate) fn set_fonts(&mut self, fonts: Arc<dyn FontRegistry>) {
        self.cache.set_font_registry(Arc::clone(&fonts));
        self.fonts = Some(fonts);
    }

    /// Reports whether glyphs can be rasterised.
    pub(crate) fn rasterises(&self) -> bool {
        self.raster.is_some()
    }

    /// Returns how many glyph bitmaps the cache has rasterised so far.
    pub(crate) fn rasterised(&self) -> u64 {
        self.cache.stats().glyphs_rasterised
    }

    /// Returns the bytes of the atlas pages, and the bytes of the pinned tiles among them.
    pub(crate) fn atlas_bytes(&self) -> (u64, u64) {
        let memory = self.cache.memory();
        (memory.atlas_bytes, memory.atlas_pinned_bytes)
    }

    /// Unpins the tiles of a lowering made in content generation `epoch`.
    pub(crate) fn unpin(&mut self, epoch: u64, tiles: &[TileKey]) {
        if epoch == self.epoch {
            self.cache.unpin(tiles);
        }
    }

    /// Drops every tile and records the destruction of every page.
    ///
    /// Every lowering of an earlier generation must be lowered again. Returns the bytes of the
    /// pages destroyed.
    pub(crate) fn forget(&mut self) -> u64 {
        let freed = self.cache.forget_caches();

        self.drain();
        self.restart();
        freed
    }

    /// Advances the generation after the cache dropped its tiles.
    fn restart(&mut self) {
        self.epoch += 1;
        self.masks.clear();
    }

    /// Moves the cache's new page operations into the log.
    pub(crate) fn drain(&mut self) {
        let mut ops = Vec::new();
        self.cache.drain_page_ops(&mut ops);
        self.pages.extend(ops);
    }

    /// Prepares the cache for a frame of a renderer with `caps`, and returns whether every
    /// lowering of an earlier generation must be lowered again.
    ///
    /// The cache starts over when the renderer allows smaller pages than the cache makes. A
    /// renderer that holds no page is given every page at the end of the frame.
    pub(crate) fn prepare(
        &mut self,
        renderer: &mut RendererState,
        caps: &RenderCapabilities,
    ) -> bool {
        let epoch = self.epoch;
        if caps.max_texture_size < self.cache.config().max_page_size {
            let mut cache = ContentCache::new(ContentConfig::for_capabilities(caps));
            if let Some(raster) = &self.raster {
                cache.set_glyph_raster(Arc::clone(raster));
            }
            if let Some(fonts) = &self.fonts {
                cache.set_font_registry(Arc::clone(fonts));
            }
            self.cache.forget_caches();
            self.drain();
            self.cache = cache;
            self.restart();
        }

        if renderer.fresh {
            renderer.pages = PageCursor::replaying();
        }

        self.cache.begin_frame();
        epoch != self.epoch
    }

    /// Ends a frame, enforcing the atlas budget and adding the page operations the renderer has
    /// not received to `work`.
    ///
    /// A renderer that holds no page is sent a creation and the texels of every page that exists,
    /// so nothing is rasterised again for it.
    pub(crate) fn finish(
        &mut self,
        renderer: &mut RendererState,
        budget: Option<u64>,
        work: &mut ResourceWork,
    ) {
        self.cache.enforce_budget(budget);
        self.cache.end_frame();
        self.drain();

        if renderer.pages.replay {
            self.cache.replay_pages(&self.images, &mut work.pages);
            self.pages.catch_up(&mut renderer.pages);
        } else {
            self.pages.send(&mut renderer.pages, &mut work.pages);
        }
    }
}
