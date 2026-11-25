//! Small solid paths rasterised into mono masks, with the history that keeps changing paths off
//! them.

mod budget;
mod density;
mod fingerprint;
mod forget;
mod history;
mod lower;
mod shape;

use std::sync::Arc;

use iceblink::{AtlasTile, ChunkKey, ClientId};
use iceblink_alloc::atlas::{AtlasError, AtlasKey, TextureKind, UploadBytes};
use kurbo::{Affine, BezPath, PathEl};
use rustc_hash::FxHashMap;

use crate::content::key::TileKey;
use crate::content::mask::budget::Budget;
use crate::content::mask::fingerprint::{Fingerprint, raster};
use crate::content::mask::history::{Observed, RouteHistory};
use crate::content::tiles::TileStore;

pub use self::lower::PathRoute;

/// The most segments a volatile path may have and keep masks.
// Past this, a raster per frame costs more than the path rasteriser does.
const VOLATILE_SEGMENTS: usize = 4096;

/// A path item across lowerings: its client, its chunk's key and its position among the chunk's
/// items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathOwner {
    /// The client the path's chunk belongs to.
    pub client: ClientId,
    /// The key of the path's chunk.
    pub chunk: ChunkKey,
    /// The path's position among the chunk's items.
    pub position: u32,
}

/// One part of a path a mask is asked for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Part<'a> {
    /// The interior, by the even-odd rule when set.
    Fill(bool),
    /// The outline of a stroke.
    Stroke(&'a kurbo::Stroke),
}

/// The geometry of one mask.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MaskRequest<'a> {
    pub(crate) path: &'a BezPath,
    /// Maps path space into local space.
    pub(crate) placement: Affine,
    pub(crate) part: Part<'a>,
    /// Mask texels per local unit along each axis.
    pub(crate) density: [f32; 2],
    /// The sub-pixel shift that aligns mask texels with whole device pixels.
    pub(crate) phase: [f32; 2],
    /// The mask's texels, as `[x, y, width, height]` in mask space.
    pub(crate) bounds: [i32; 4],
}

/// The answer to one mask's tile request.
#[derive(Clone, Copy, Debug)]
pub(crate) enum MaskAnswer {
    /// The mask's tile.
    Tile(AtlasTile, TileKey),
    /// The atlas cannot hold the mask.
    Refused,
    /// The pool is full. Evicting and asking again may place it.
    NoRoom,
}

/// The masks of the paths lowered lately, and each path's recent history.
#[derive(Debug, Default)]
pub(crate) struct MaskCache {
    entries: FxHashMap<Arc<Fingerprint>, TileKey>,
    by_tile: FxHashMap<AtlasKey, Arc<Fingerprint>>,
    histories: FxHashMap<PathOwner, RouteHistory>,
    /// The frames counted so far.
    frame: u32,
    /// Whether a path that became a frame path keeps its history, as of the last frame counted
    /// or a later decline.
    waiting: bool,
    /// Masks their paths stopped drawing, removed when the frame ends.
    superseded: Vec<TileKey>,
    budget: Budget,
    pub(crate) rasterised: u64,
    pub(crate) declined: u64,
}

impl MaskCache {
    /// Starts a frame that lowers paths, with a fresh budget.
    pub(crate) fn begin_frame(&mut self) {
        self.budget = Budget::default();
    }

    /// Ends a frame, and returns the keys of the masks it removed.
    ///
    /// Superseded masks that are unpinned and unused in this frame are removed.
    pub(crate) fn end_frame(&mut self, tiles: &mut TileStore) -> Vec<AtlasKey> {
        let mut removed = Vec::new();
        for key in self.superseded.drain(..) {
            if tiles.remove_if_unused(key) {
                removed.push(key.atlas());
            }
        }
        removed
    }

    /// Observes the masks of path `owner`, one per part it has, and reports whether the path is
    /// drawn with them in this frame.
    ///
    /// The route depends on the observation:
    ///
    /// - If no part asks for a mask other than its most recent one, the path takes the route it
    ///   took last.
    /// - If the path is not volatile, it takes masks.
    /// - Otherwise it becomes a frame path when `path` has more than [`VOLATILE_SEGMENTS`]
    ///   segments, or when its new masks, those its parts did not keep, would take the frame past
    ///   its budget.
    ///
    /// The tiles a declined path tracks are removed at the frame's end unless they are pinned or
    /// used.
    fn route(
        &mut self,
        owner: PathOwner,
        masks: [Option<&Fingerprint>; 2],
        path: &BezPath,
    ) -> bool {
        let frame = self.frame;
        let history = self
            .histories
            .entry(owner)
            .or_insert_with(|| RouteHistory::new(frame));

        let digests = masks.map(|mask| mask.map(Fingerprint::digest));
        if let Observed::Differs(fresh) = history.observe(frame, digests) {
            let new = masks
                .into_iter()
                .zip(fresh)
                .filter_map(|(mask, fresh)| mask.filter(|_| fresh));
            let (tiles, texels) = new.fold((0, 0), |(tiles, texels), mask| {
                (tiles + 1, texels + mask.texels())
            });
            history.declined = history.volatile
                && (segments(path) > VOLATILE_SEGMENTS || !self.budget.spend(tiles, texels));
            if history.declined {
                history.give_back(&mut self.superseded);
                self.waiting = true;
            }
        }

        if history.declined {
            self.declined += 1;
        }
        !history.declined
    }

    /// Makes `key` the current tile of `part` of path `owner`.
    fn track(&mut self, owner: PathOwner, part: usize, key: TileKey) {
        if let Some(history) = self.histories.get_mut(&owner) {
            history.track(part, key, &mut self.superseded);
        }
    }

    /// Returns the tile of `mask`, rasterising it on a miss.
    fn tile_for(&mut self, tiles: &mut TileStore, mask: &Arc<Fingerprint>) -> MaskAnswer {
        let held = self.entries.get(&**mask).copied();
        if let Some(key) = held
            && let Some(tile) = tiles.get(key)
        {
            return MaskAnswer::Tile(tile, key);
        }

        let key = held.unwrap_or_else(|| tiles.fresh_key(TextureKind::Mono));
        let mut bytes = Some(UploadBytes::Owned(raster(mask)));
        let tile = match tiles.insert(key, mask.size, &mut bytes) {
            Ok(tile) => tile,
            Err(AtlasError::OutOfSpace { .. }) => return MaskAnswer::NoRoom,
            Err(_) => return MaskAnswer::Refused,
        };

        self.rasterised += 1;
        if held.is_none() {
            self.by_tile.insert(key.atlas(), Arc::clone(mask));
            self.entries.insert(Arc::clone(mask), key);
        }
        MaskAnswer::Tile(tile, key)
    }

    /// Forgets the masks whose tiles were removed.
    pub(crate) fn forget_tiles(&mut self, removed: &[AtlasKey]) {
        for key in removed {
            if let Some(fingerprint) = self.by_tile.remove(key) {
                self.entries.remove(&fingerprint);
            }
        }
    }

    /// Forgets every mask and history.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.by_tile.clear();
        self.histories.clear();
        self.waiting = false;
        self.superseded.clear();
    }
}

/// Returns the number of drawing segments of `path`.
fn segments(path: &BezPath) -> usize {
    path.elements()
        .iter()
        .filter(|element| {
            matches!(
                element,
                PathEl::LineTo(_) | PathEl::QuadTo(..) | PathEl::CurveTo(..)
            )
        })
        .count()
}
