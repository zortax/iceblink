//! Marks items lowered into frame marks.

use std::sync::Arc;

use iceblink::{
    ClipRef, FrameMarkPayload, FrameMarks, FramePath, MarkCell, MarkPayload, Marks, SpaceRef,
};
use iceblink_alloc::atlas::{AtlasError, TextureKind, UploadBytes};

use crate::content::cache::ContentCache;
use crate::content::error::ContentError;
use crate::content::key::TileKey;
use crate::content::marks::PlacedSheet;
use crate::content::marks::layout::PHASES;
use crate::content::marks::paths::large_paths;
use crate::content::marks::sheet::{self, SheetKey, frame_cells};
use crate::content::target::Target;

/// A marks item lowered for a frame.
#[derive(Clone, Debug)]
pub struct LoweredMarks {
    /// The item with frame slots and the chunk offset applied, and the page of its cells.
    ///
    /// Its `payload` index is the item's, unchanged.
    pub marks: FrameMarks,
    /// The payload with the cells of its shapes.
    pub payload: FrameMarkPayload,
    /// The tile the cells lie in, or `None` for a payload without shapes.
    pub tile: Option<TileKey>,
    /// The placements whose shapes are too large for a cell, as filled paths in paint order, or one
    /// path for an item with `UNION`.
    pub paths: Vec<FramePath>,
}

impl ContentCache {
    /// Lowers a marks item drawing `payload` at `target`.
    ///
    /// The payload's shapes are rasterised once into one tile, at the sixteen quarter-pixel
    /// phases of each, and the frame's cells index that tile. A payload whose shapes equal those
    /// of a payload lowered before shares its tile.
    ///
    /// A shape whose cell would be wider or taller than a quarter of a page gets empty cells, and
    /// its placements are lowered into paths at their quantised anchors instead. Under
    /// [`MarkFlags::UNION`](iceblink::MarkFlags::UNION) the item's paths form one path, so their
    /// overlaps blend once.
    ///
    /// # Errors
    ///
    /// Returns [`ContentError::Atlas`] if no tile was found after evicting the oldest unpinned
    /// tiles once.
    pub fn lower_marks(
        &mut self,
        item: &Marks,
        payload: &Arc<MarkPayload>,
        target: &Target,
    ) -> Result<LoweredMarks, ContentError> {
        let [ox, oy] = target.offset;
        let mut marks = *item;
        marks.bounds[0] += ox;
        marks.bounds[1] += oy;
        marks.origin = [item.origin[0] + ox, item.origin[1] + oy];
        marks.paint_origin = [item.paint_origin[0] + ox, item.paint_origin[1] + oy];
        marks.clip = ClipRef(target.clip);
        marks.space = SpaceRef(target.space);

        if payload.shapes.is_empty() {
            return Ok(LoweredMarks {
                marks: FrameMarks { marks, page: 0 },
                payload: FrameMarkPayload {
                    payload: Arc::clone(payload),
                    cells: Arc::clone(&self.marks.no_cells),
                },
                tile: None,
                paths: Vec::new(),
            });
        }

        let sheet = self.sheet_for(payload)?;
        let mut paths = Vec::new();
        if sheet.large.contains(&true) {
            large_paths(&marks, payload, &sheet.large, &target.world, &mut paths);
        }
        Ok(LoweredMarks {
            marks: FrameMarks {
                marks,
                page: sheet.page,
            },
            payload: FrameMarkPayload {
                payload: Arc::clone(payload),
                cells: sheet.cells,
            },
            tile: Some(sheet.key),
            paths,
        })
    }

    /// Returns the placed sheet of `payload`'s shapes, rasterising and placing it on a miss.
    fn sheet_for(&mut self, payload: &Arc<MarkPayload>) -> Result<PlacedSheet, ContentError> {
        if let Some(key) = self.marks.key_of(payload)
            && let Some(sheet) = self.marks.sheet(key)
            && self.tiles.get(sheet.key).is_some()
        {
            return Ok(sheet.clone());
        }

        let key = SheetKey::of(&payload.shapes);
        if let Some(sheet) = self.marks.sheet(&key).cloned()
            && self.tiles.get(sheet.key).is_some()
        {
            self.marks.remember_payload(payload, key);
            return Ok(sheet);
        }

        let page = self.config.page_size.min(self.config.max_page_size);
        let max_cell = (page / PHASES as u32).max(1);
        let raster = sheet::rasterise(&payload.shapes, max_cell, page);
        self.marks.rasterised += 1;
        let tile_key = self.tiles.fresh_key(TextureKind::Mono);
        let mut bytes = Some(UploadBytes::Owned(raster.texels));
        let tile = match self.tiles.insert(tile_key, raster.size, &mut bytes) {
            Err(AtlasError::OutOfSpace { .. }) if self.evict_oldest() => {
                self.tiles.insert(tile_key, raster.size, &mut bytes)
            }
            tile => tile,
        }?;

        let cells: Arc<[MarkCell]> = frame_cells(&raster.cells, [tile.rect[0], tile.rect[1]]);
        let sheet = PlacedSheet {
            key: tile_key,
            page: tile.page,
            cells,
            large: raster.large.into(),
        };
        self.marks.insert(key.clone(), sheet.clone());
        self.marks.remember_payload(payload, key);
        Ok(sheet)
    }
}
