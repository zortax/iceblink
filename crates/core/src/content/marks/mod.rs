//! Mark shapes rasterised at every quarter-pixel phase, and marks items lowered onto them.

mod layout;
mod lower;
mod paths;
mod sheet;

use std::sync::{Arc, Weak};

use iceblink::{MarkCell, MarkPayload};
use iceblink_alloc::atlas::AtlasKey;
use rustc_hash::FxHashMap;

use crate::content::key::TileKey;

pub use self::lower::LoweredMarks;
pub(crate) use self::sheet::SheetKey;

/// How many payloads are remembered by address before dead ones are dropped.
const PAYLOADS_KEPT: usize = 1_024;

/// A placed sheet: its tile, its page and its frame cells.
#[derive(Clone, Debug)]
pub(crate) struct PlacedSheet {
    pub(crate) key: TileKey,
    pub(crate) page: u32,
    pub(crate) cells: Arc<[MarkCell]>,
    /// Whether each shape is too large for a cell, so its placements are drawn as paths.
    pub(crate) large: Arc<[bool]>,
}

/// The sheets of the mark shapes drawn so far.
///
/// A sheet is kept by the exact geometry of its payload's shapes, so payloads with equal shapes
/// share it. A payload is also remembered by its address, so lowering it again reads no geometry.
#[derive(Debug)]
pub(crate) struct MarkSheets {
    by_shapes: FxHashMap<SheetKey, PlacedSheet>,
    by_payload: FxHashMap<usize, (Weak<MarkPayload>, SheetKey)>,
    by_tile: FxHashMap<AtlasKey, SheetKey>,
    /// Sheets rasterised since the cache was made.
    pub(crate) rasterised: u64,
    /// The empty cell list every payload without shapes shares.
    pub(crate) no_cells: Arc<[MarkCell]>,
}

impl Default for MarkSheets {
    fn default() -> Self {
        Self {
            by_shapes: FxHashMap::default(),
            by_payload: FxHashMap::default(),
            by_tile: FxHashMap::default(),
            rasterised: 0,
            no_cells: Arc::from([]),
        }
    }
}

impl MarkSheets {
    /// Returns the sheet key `payload` was last lowered with.
    pub(crate) fn key_of(&self, payload: &Arc<MarkPayload>) -> Option<&SheetKey> {
        let (weak, key) = self.by_payload.get(&(Arc::as_ptr(payload) as usize))?;
        // A dead payload's address can come back as a different payload.
        (weak.strong_count() > 0 && core::ptr::eq(weak.as_ptr(), Arc::as_ptr(payload)))
            .then_some(key)
    }

    /// Returns the sheet of `key`.
    pub(crate) fn sheet(&self, key: &SheetKey) -> Option<&PlacedSheet> {
        self.by_shapes.get(key)
    }

    /// Records that `payload` draws the sheet of `key`.
    pub(crate) fn remember_payload(&mut self, payload: &Arc<MarkPayload>, key: SheetKey) {
        if self.by_payload.len() >= PAYLOADS_KEPT {
            self.by_payload
                .retain(|_, (weak, _)| weak.strong_count() > 0);
        }
        self.by_payload.insert(
            Arc::as_ptr(payload) as usize,
            (Arc::downgrade(payload), key),
        );
    }

    /// Records a newly placed sheet.
    pub(crate) fn insert(&mut self, key: SheetKey, sheet: PlacedSheet) {
        self.by_tile.insert(sheet.key.atlas(), key.clone());
        if let Some(previous) = self.by_shapes.insert(key, sheet) {
            self.by_tile.remove(&previous.key.atlas());
        }
    }

    /// Forgets the sheets whose tiles were removed.
    pub(crate) fn forget_tiles(&mut self, removed: &[AtlasKey]) {
        for tile in removed {
            if let Some(key) = self.by_tile.remove(tile) {
                self.by_shapes.remove(&key);
            }
        }
    }

    /// Forgets every sheet and payload.
    pub(crate) fn clear(&mut self) {
        self.by_shapes.clear();
        self.by_payload.clear();
        self.by_tile.clear();
    }
}
