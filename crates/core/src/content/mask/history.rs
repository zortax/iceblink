//! What the mask cache remembers about each path between frames.

use crate::content::key::TileKey;

/// The frames the volatile test reads: this one and the three before it.
const RECENT: u8 = 0b1111;

/// The four frames before this one, a change in which keeps a volatile path volatile.
const BEFORE: u8 = 0b1_1110;

/// How many changes in [`RECENT`] make a path volatile.
const VOLATILE_CHANGES: u32 = 3;

/// How many changes in eight frames make a path's dropped masks worth removing at the frame's
/// end.
const RECLAIM_CHANGES: u32 = 2;

/// How many frames without a difference forget a history.
const HISTORY_FRAMES: u32 = 8;

/// The recent masks of one path, for its fill and its stroke, how often they changed and the
/// route the path took.
#[derive(Clone, Copy, Debug)]
pub(super) struct RouteHistory {
    /// Bit `n` is set when the path changed `n` frames before `since`.
    changes: u8,
    /// The last frame a part's mask differed from the part's most recent one.
    since: u32,
    /// Whether the path changes too often for masks.
    pub(super) volatile: bool,
    /// Whether the path became a frame path the last time its masks differed.
    pub(super) declined: bool,
    /// The digests of each part's two most recent distinct masks, the most recent first.
    kept: [[Option<u64>; 2]; 2],
    /// The tiles each part drew last, the current one first.
    tiles: [[Option<TileKey>; 2]; 2],
}

/// What one observation found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Observed {
    /// No part asked for a mask other than its most recent one.
    Same,
    /// A part asked for another mask, and the parts set here asked for a new one: neither of
    /// the two the part kept.
    Differs([bool; 2]),
}

impl RouteHistory {
    /// Returns a history that has seen nothing, in `frame`.
    pub(super) fn new(frame: u32) -> Self {
        Self {
            changes: 0,
            since: frame,
            volatile: false,
            declined: false,
            kept: [[None; 2]; 2],
            tiles: [[None; 2]; 2],
        }
    }

    /// Observes the digests of the path's masks in `frame`, one per part it has.
    ///
    /// The path changes in `frame` when a part that kept a mask asks for a new
    /// one. It turns volatile once it changed in three of the last four frames, and stays
    /// volatile until four frames in a row pass without a change. Frames without a difference
    /// count as frames without a change.
    pub(super) fn observe(&mut self, frame: u32, masks: [Option<u64>; 2]) -> Observed {
        let differs = masks
            .iter()
            .zip(&self.kept)
            .any(|(mask, kept)| mask.is_some() && *mask != kept[0]);
        if !differs {
            return Observed::Same;
        }
        self.advance(frame);

        let mut fresh = [false; 2];
        let mut changed = false;
        for ((mask, kept), fresh) in masks.into_iter().zip(&mut self.kept).zip(&mut fresh) {
            let Some(mask) = mask else {
                continue;
            };
            if kept[0] == Some(mask) {
                continue;
            }
            if kept[1] == Some(mask) {
                kept.swap(0, 1);
                continue;
            }

            *fresh = true;
            changed |= kept[0].is_some();
            *kept = [Some(mask), kept[0]];
        }

        if changed {
            self.changes |= 1;
        }
        let recent = (self.changes & RECENT).count_ones();
        self.volatile = recent >= VOLATILE_CHANGES || (self.volatile && recent > 0);
        Observed::Differs(fresh)
    }

    /// Makes `key` the current tile of `part`.
    ///
    /// The tile before it stays tracked, so a path that toggles between two shapes keeps hitting.
    /// The one before that is pushed to `superseded` when the path changed at least twice in eight
    /// frames, and is left to eviction otherwise.
    pub(super) fn track(&mut self, part: usize, key: TileKey, superseded: &mut Vec<TileKey>) {
        let tiles = &mut self.tiles[part];
        if tiles[0] == Some(key) {
            return;
        }
        if tiles[1] == Some(key) {
            tiles.swap(0, 1);
            return;
        }

        let dropped = tiles[1];
        *tiles = [Some(key), tiles[0]];
        if let Some(dropped) = dropped
            && self.changes.count_ones() >= RECLAIM_CHANGES
        {
            superseded.push(dropped);
        }
    }

    /// Moves every tile the history tracks into `superseded`.
    pub(super) fn give_back(&mut self, superseded: &mut Vec<TileKey>) {
        let tiles = self.tiles.iter_mut().flatten();
        superseded.extend(tiles.filter_map(Option::take));
    }

    /// Reports whether no mask differed for [`HISTORY_FRAMES`] frames by `frame`.
    pub(super) fn stale(&self, frame: u32) -> bool {
        frame.wrapping_sub(self.since) >= HISTORY_FRAMES
    }

    /// Shifts the change register to `frame`, and ends a volatile spell that the frames since
    /// the last difference closed.
    fn advance(&mut self, frame: u32) {
        let gap = frame.wrapping_sub(self.since);
        if gap == 0 {
            return;
        }

        self.changes = if gap >= u8::BITS {
            0
        } else {
            self.changes << gap
        };
        if self.changes & BEFORE == 0 {
            self.volatile = false;
        }
        self.since = frame;
    }
}
