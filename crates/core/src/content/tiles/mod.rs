//! The atlas, its page operations and the pins on its tiles.

mod evict;
mod insert;

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{AtlasTile, PageOp};
use iceblink_alloc::atlas::{Atlas, AtlasKey, AtlasLimits, TextureKind};

use crate::content::key::TileKey;
use crate::content::pages::{PageSink, page_of};

/// The atlas behind every tile, with the page operations its work produces.
///
/// Page operations come out in the order the atlas performs them, and an upload always follows
/// the creation of its page.
#[derive(Debug)]
pub(crate) struct TileStore {
    atlas: Atlas,
    sink: PageSink,
    next_handle: u64,
    /// Tiles removed by any path, since the store was made.
    evicted: u64,
    /// The rectangle of each tile, so an upload into part of a tile can be placed.
    rects: rustc_hash::FxHashMap<AtlasKey, AtlasTile>,
}

impl TileStore {
    /// Creates an empty store that allocates within `limits`.
    pub(crate) fn new(limits: AtlasLimits) -> Self {
        Self {
            atlas: Atlas::new(limits),
            sink: PageSink::default(),
            next_handle: 0,
            evicted: 0,
            rects: rustc_hash::FxHashMap::default(),
        }
    }

    /// Returns a key no tile has had before, in the pool of `kind`.
    pub(crate) fn fresh_key(&mut self, kind: TextureKind) -> TileKey {
        self.next_handle += 1;
        TileKey(AtlasKey::new(self.next_handle, kind))
    }

    /// Starts a new generation, in which no tile is used yet.
    pub(crate) fn begin_frame(&mut self) {
        self.atlas.begin_frame();
    }

    /// Returns the tile of `key` and marks it used in the current generation.
    pub(crate) fn get(&mut self, key: TileKey) -> Option<AtlasTile> {
        self.atlas.get(key.atlas())?;
        self.rects.get(&key.atlas()).copied()
    }

    /// Returns the tile of `key` without marking it used.
    pub(crate) fn peek(&self, key: TileKey) -> Option<AtlasTile> {
        if !self.atlas.contains(key.atlas()) {
            return None;
        }
        self.rects.get(&key.atlas()).copied()
    }

    /// Appends a creation of every page, followed by the texels of each page that does not hold
    /// images.
    pub(crate) fn replay(&self, out: &mut Vec<PageOp>) {
        self.sink.replay(out);
    }

    /// Returns the bytes of the page copies kept for [`TileStore::replay`].
    pub(crate) fn copied_bytes(&self) -> u64 {
        self.sink.copied_bytes()
    }

    /// Reports whether `key` holds a tile, without marking it used.
    pub(crate) fn contains(&self, key: TileKey) -> bool {
        self.atlas.contains(key.atlas())
    }

    /// Writes `bytes` into the rectangle `within` of level `level` of the tile of `key`.
    ///
    /// `within` is measured from the tile's corner, in texels of the level. Only a tile on a page
    /// of its own has levels below 0. Nothing is written when `key` holds no tile.
    pub(crate) fn upload(
        &mut self,
        key: TileKey,
        level: u32,
        within: [u32; 4],
        bytes: Arc<[u8]>,
        offset: usize,
        stride: u32,
    ) {
        let Some(tile) = self.rects.get(&key.atlas()).copied() else {
            return;
        };

        self.flush();
        let rect = Rect::new(
            Point::new(
                ((tile.rect[0] >> level) + within[0]) as i32,
                ((tile.rect[1] >> level) + within[1]) as i32,
            ),
            Size::new(within[2] as i32, within[3] as i32),
        );
        self.sink
            .push_upload(tile.page, level, rect, bytes, offset, stride);
    }

    /// Pins `key` against eviction.
    pub(crate) fn pin(&mut self, key: TileKey) {
        self.atlas.retain(key.atlas());
    }

    /// Releases one pin of `key`.
    pub(crate) fn unpin(&mut self, key: TileKey) {
        self.atlas.release(key.atlas());
    }

    /// Returns the bytes of the atlas pages.
    pub(crate) fn bytes(&self) -> u64 {
        self.atlas.resident_bytes()
    }

    /// Returns the tile bytes of the pinned tiles.
    pub(crate) fn pinned_bytes(&self) -> u64 {
        self.atlas.report().referenced_bytes
    }

    /// Returns the number of tiles.
    pub(crate) fn len(&self) -> usize {
        self.atlas.len()
    }

    /// Returns the tiles removed since the store was made.
    pub(crate) fn evicted(&self) -> u64 {
        self.evicted
    }

    /// Returns the insertions the atlas refused, since the store was made.
    pub(crate) fn refusals(&self) -> u64 {
        self.atlas.refusals()
    }

    /// Returns the pages created, the pages destroyed and the texels uploaded since the store was
    /// made.
    pub(crate) fn page_counts(&self) -> (u64, u64, u64) {
        (
            self.sink.pages_created(),
            self.sink.pages_destroyed(),
            self.sink.uploaded_texels(),
        )
    }

    /// Moves every page operation to the end of `out`, oldest first.
    pub(crate) fn drain_ops(&mut self, out: &mut Vec<PageOp>) {
        self.flush();
        self.sink.drain_into(out);
    }

    /// Drops every page operation, including the atlas's queued work.
    pub(crate) fn discard_ops(&mut self) {
        let mut dropped = Vec::new();
        self.drain_ops(&mut dropped);
    }

    /// Reports whether page operations are waiting.
    pub(crate) fn has_ops(&self) -> bool {
        !self.sink.is_empty()
            || self.atlas.pending_textures() > 0
            || self.atlas.report().pending_uploads > 0
    }

    /// Performs the atlas's queued work against the page sink.
    fn flush(&mut self) {
        // The page sink accepts every operation, so the flush cannot fail.
        let flushed = self.atlas.flush_uploads(&mut self.sink);
        debug_assert!(flushed.is_ok(), "the page sink refused an operation");
    }

    /// Records the rectangle of a newly placed tile.
    fn remember(&mut self, key: TileKey, tile: iceblink_alloc::atlas::AtlasTile) -> AtlasTile {
        let tile = frame_tile(tile);
        self.rects.insert(key.atlas(), tile);
        tile
    }
}

/// Converts an allocator tile into a frame's atlas tile: its page and texel rectangle.
fn frame_tile(tile: iceblink_alloc::atlas::AtlasTile) -> AtlasTile {
    AtlasTile {
        page: page_of(tile.texture),
        rect: [
            tile.bounds.origin.x.max(0) as u32,
            tile.bounds.origin.y.max(0) as u32,
            tile.bounds.size.width.max(0) as u32,
            tile.bounds.size.height.max(0) as u32,
        ],
    }
}
