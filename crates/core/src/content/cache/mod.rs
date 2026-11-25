//! The content cache: the atlas and everything rasterised into it.

mod budget;
mod pins;

use std::sync::Arc;

use iceblink::{
    ChunkKey, ClientId, FontData, FontId, FontRegistry, GlyphBitmap, GlyphRaster, PageOp,
};
use iceblink_alloc::atlas::AtlasKey;

use crate::content::config::ContentConfig;
use crate::content::glyph::GlyphState;
use crate::content::image::ImageTiles;
use crate::content::marks::MarkSheets;
use crate::content::mask::{MaskCache, PathOwner};
use crate::content::memory::ContentMemory;
use crate::content::stats::ContentStats;
use crate::content::tiles::TileStore;

/// Atlas pages and the content rasterised into them.
///
/// The cache lowers glyph runs, images, marks and paths into frame instances. It rasterises
/// what it has not seen, places it in atlas tiles and records every page creation, upload and
/// destruction as a [`PageOp`].
///
/// # Pins and eviction
///
/// Every lowering appends the keys of the tiles it read. A tile stays while it is pinned or used
/// in the current frame, which [`ContentCache::begin_frame`] starts. Eviction removes the other
/// tiles, least recently used first, and destroys the pages it empties. Instances lowered before
/// an eviction may read removed tiles, which [`ContentCache::is_resident`] and
/// [`ContentCache::evictions`] detect.
///
/// A tile must stay while a renderer keeps resident instances that read it, while the last frame
/// of an output drew it, and while the frame being built draws it. Use in the current frame
/// covers only the last of these, so the other two need pins.
///
/// ```
/// use iceblink_core::content::{ContentCache, ContentConfig};
///
/// let mut cache = ContentCache::new(ContentConfig::default());
/// cache.begin_frame();
/// let mut ops = Vec::new();
/// cache.drain_page_ops(&mut ops);
/// assert!(ops.is_empty());
/// assert_eq!(cache.memory().atlas_bytes, 0);
/// ```
pub struct ContentCache {
    pub(crate) config: ContentConfig,
    pub(crate) tiles: TileStore,
    pub(crate) glyphs: GlyphState,
    pub(crate) images: ImageTiles,
    pub(crate) marks: MarkSheets,
    pub(crate) masks: MaskCache,
    pub(crate) raster: Option<Arc<dyn GlyphRaster>>,
    pub(crate) fonts: Option<Arc<dyn FontRegistry>>,
    /// A bitmap the serial rasteriser reuses.
    pub(crate) bitmap: GlyphBitmap,
    /// The counts that the parts of the cache do not keep themselves.
    pub(crate) tally: ContentStats,
    /// Keys removed by the last eviction, kept for the allocation.
    removed: Vec<AtlasKey>,
}

impl core::fmt::Debug for ContentCache {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ContentCache")
            .field("config", &self.config)
            .field("memory", &self.memory())
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl ContentCache {
    /// Creates an empty cache whose pages stay within `config`.
    pub fn new(config: ContentConfig) -> Self {
        Self {
            config,
            tiles: TileStore::new(config.limits()),
            glyphs: GlyphState::default(),
            images: ImageTiles::default(),
            marks: MarkSheets::default(),
            masks: MaskCache::default(),
            raster: None,
            fonts: None,
            bitmap: GlyphBitmap::default(),
            tally: ContentStats::default(),
            removed: Vec::new(),
        }
    }

    /// Returns the configuration the cache was created with.
    pub fn config(&self) -> ContentConfig {
        self.config
    }

    /// Sets the rasteriser glyphs are drawn with.
    ///
    /// Without one, glyph runs lower into nothing. Glyphs already rasterised stay.
    pub fn set_glyph_raster(&mut self, raster: Arc<dyn GlyphRaster>) {
        self.raster = Some(raster);
    }

    /// Sets the registry fonts are read from.
    ///
    /// Without one, glyph runs lower into nothing.
    pub fn set_font_registry(&mut self, fonts: Arc<dyn FontRegistry>) {
        self.fonts = Some(fonts);
    }

    /// Starts a frame.
    ///
    /// Tiles a lowering or [`ContentCache::touch`] marks in this frame are kept by eviction
    /// until the next call.
    pub fn begin_frame(&mut self) {
        self.tiles.begin_frame();
        self.glyphs.outlines.begin_frame();
        self.masks.begin_frame();
    }

    /// Counts a frame of the route histories of paths, whether it lowers anything or not.
    ///
    /// Histories whose masks did not differ for eight frames are forgotten. The owners of the
    /// frame paths among them are appended to `returning`, since those paths take masks when they
    /// are next lowered.
    pub fn count_frame(&mut self, returning: &mut Vec<PathOwner>) {
        self.masks.count_frame(returning);
    }

    /// Reports whether a path that became a frame path keeps its route history.
    pub fn routes_waiting(&self) -> bool {
        self.masks.waiting()
    }

    /// Forgets the route history of the path at `position` of chunk `chunk` of `client`.
    pub fn forget_path(&mut self, client: ClientId, chunk: ChunkKey, position: u32) {
        self.masks.forget_path(client, chunk, position);
    }

    /// Forgets the route history of every path of `client`.
    pub fn forget_client(&mut self, client: ClientId) {
        self.masks.forget_client(client);
    }

    /// Ends a frame.
    ///
    /// Path masks replaced in this frame are removed, unless they are pinned or used.
    pub fn end_frame(&mut self) {
        let removed = self.masks.end_frame(&mut self.tiles);
        self.forget(&removed);
    }

    /// Moves every page operation to the end of `out`, in the order the renderer applies them.
    pub fn drain_page_ops(&mut self, out: &mut Vec<PageOp>) {
        self.tiles.drain_ops(out);
    }

    /// Reports whether page operations are waiting for [`ContentCache::drain_page_ops`].
    pub fn has_page_ops(&self) -> bool {
        self.tiles.has_ops()
    }

    /// Returns the bytes the atlas holds.
    pub fn memory(&self) -> ContentMemory {
        ContentMemory {
            atlas_bytes: self.tiles.bytes(),
            atlas_pinned_bytes: self.tiles.pinned_bytes(),
        }
    }

    /// Returns the counts of the cache's work.
    pub fn stats(&self) -> ContentStats {
        let (pages_created, pages_destroyed, texels_uploaded) = self.tiles.page_counts();
        ContentStats {
            glyphs_rebuilt: self.glyphs.answers.rebuilt(),
            outlines_written: self.glyphs.outlines.written(),
            tiles_evicted: self.tiles.evicted(),
            atlas_refusals: self.tiles.refusals(),
            pages_created,
            pages_destroyed,
            texels_uploaded,
            live_tiles: self.tiles.len() as u64,
            live_glyphs: self.glyphs.answers.len() as u64,
            masks_rasterised: self.masks.rasterised,
            masks_declined: self.masks.declined,
            mark_sheets_rasterised: self.marks.rasterised,
            ..self.tally
        }
    }

    /// Returns the font of `id` from the registry.
    pub(crate) fn font(&self, id: FontId) -> Option<FontData> {
        self.fonts.as_ref()?.get(id)
    }

    /// Removes the unpinned tiles of the oldest generation, and reports whether any were removed.
    pub(crate) fn evict_oldest(&mut self) -> bool {
        let mut removed = core::mem::take(&mut self.removed);
        removed.clear();
        let evicted = self.tiles.evict_oldest(&mut removed);

        self.forget(&removed);
        self.removed = removed;
        evicted
    }

    /// Forgets what every part of the cache knew about removed tiles.
    pub(crate) fn forget(&mut self, removed: &[AtlasKey]) {
        if removed.is_empty() {
            return;
        }

        self.glyphs.answers.forget_tiles(removed);
        self.masks.forget_tiles(removed);
        self.marks.forget_tiles(removed);
        self.images.forget_tiles(removed);
    }
}
