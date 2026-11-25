//! What a content cache has done, in numbers.

/// Counts of a [`ContentCache`](crate::content::ContentCache)'s work since it was created.
///
/// Every count only grows, so the difference of two readings is the work between them. The
/// `live_` fields are the state at the time of the reading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentStats {
    /// Glyphs lowered into sprites.
    pub glyphs_placed: u64,
    /// Glyphs lowered into paths.
    pub glyphs_outlined: u64,
    /// Glyph bitmaps the rasteriser produced, blank ones included.
    pub glyphs_rasterised: u64,
    /// Glyph bitmaps produced by prefetching, a part of `glyphs_rasterised`.
    pub glyphs_prefetched: u64,
    /// Prefetch batches that ran on worker threads.
    pub prefetch_batches: u64,
    /// Glyphs rasterised again because their tile had been evicted.
    pub glyphs_rebuilt: u64,
    /// Glyph outlines the rasteriser wrote.
    pub outlines_written: u64,
    /// Path coverage masks rasterised.
    pub masks_rasterised: u64,
    /// Paths a mask was declined for, so they stayed paths.
    pub masks_declined: u64,
    /// Mark shape sheets rasterised, each holding every phase of a payload's shapes.
    pub mark_sheets_rasterised: u64,
    /// Image tiles placed in the atlas.
    pub image_tiles_placed: u64,
    /// Tiles removed by eviction, trimming, releasing an image or forgetting the caches.
    pub tiles_evicted: u64,
    /// Insertions the atlas refused.
    pub atlas_refusals: u64,
    /// Pages created.
    pub pages_created: u64,
    /// Pages destroyed.
    pub pages_destroyed: u64,
    /// Texels uploaded into pages.
    pub texels_uploaded: u64,
    /// Tiles in the atlas.
    pub live_tiles: u64,
    /// Glyph keys with a remembered answer, blank glyphs included.
    pub live_glyphs: u64,
}

impl ContentStats {
    /// Calls `out` with every count by a stable name.
    pub fn report(&self, out: &mut dyn FnMut(&'static str, u64)) {
        out("content.glyphs_placed", self.glyphs_placed);
        out("content.glyphs_outlined", self.glyphs_outlined);
        out("content.glyphs_rasterised", self.glyphs_rasterised);
        out("content.glyphs_prefetched", self.glyphs_prefetched);
        out("content.prefetch_batches", self.prefetch_batches);
        out("content.glyphs_rebuilt", self.glyphs_rebuilt);
        out("content.outlines_written", self.outlines_written);
        out("content.masks_rasterised", self.masks_rasterised);
        out("content.masks_declined", self.masks_declined);
        out(
            "content.mark_sheets_rasterised",
            self.mark_sheets_rasterised,
        );
        out("content.image_tiles_placed", self.image_tiles_placed);
        out("content.tiles_evicted", self.tiles_evicted);
        out("content.atlas_refusals", self.atlas_refusals);
        out("content.pages_created", self.pages_created);
        out("content.pages_destroyed", self.pages_destroyed);
        out("content.texels_uploaded", self.texels_uploaded);
        out("content.live_tiles", self.live_tiles);
        out("content.live_glyphs", self.live_glyphs);
    }
}
