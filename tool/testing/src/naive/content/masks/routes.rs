//! The route histories that keep paths which change in most frames off path masks.

use std::collections::HashMap;

use iceblink::kurbo::{BezPath, PathEl};
use iceblink::{ChunkKey, ClientId};

use crate::naive::content::masks::Mask;

/// The most segments a volatile path may have and keep its masks.
const VOLATILE_SEGMENTS: usize = 4096;

/// The most new masks of volatile paths one frame admits.
const BUDGET_MASKS: u32 = 256;

/// The most texels those new masks may cover together.
const BUDGET_TEXELS: u64 = 1024 * 1024;

/// How many frames a history lasts once its masks stop differing.
const HISTORY_FRAMES: u64 = 8;

/// A path item across frames: its client, its chunk and its position among the chunk's items.
pub(crate) type PathId = (ClientId, ChunkKey, usize);

/// Every path's history, the frame count and what the current frame spent on new masks.
#[derive(Default)]
pub(crate) struct Routes {
    /// The `build_frame` calls so far.
    frame: u64,
    histories: HashMap<PathId, History>,
    /// The new masks of volatile paths the current frame admitted, and their texels.
    spent: (u32, u64),
}

/// The recent masks of one path, the frames it changed in and the route it took.
struct History {
    /// The frames the path changed in, oldest first.
    changes: Vec<u64>,
    /// Whether the path was volatile when its masks last differed.
    volatile: bool,
    /// Whether the path became a frame path when its masks last differed.
    declined: bool,
    /// The last frame a part's mask differed from the part's most recent one.
    since: u64,
    /// The two most recent distinct masks of the fill and of the stroke, the most recent first.
    kept: [Vec<Mask>; 2],
}

impl Routes {
    /// Starts the next frame, with nothing spent, and reports whether a frame path's history was
    /// forgotten.
    ///
    /// Histories whose masks did not differ for eight frames are forgotten.
    pub(crate) fn next_frame(&mut self) -> bool {
        self.frame += 1;
        self.spent = (0, 0);

        let frame = self.frame;
        let mut returned = false;
        self.histories.retain(|_, history| {
            let kept = frame - history.since < HISTORY_FRAMES;
            returned |= !kept && history.declined;
            kept
        });
        returned
    }

    /// Reports whether a path that became a frame path keeps its history.
    pub(crate) fn waiting(&self) -> bool {
        self.histories.values().any(|history| history.declined)
    }

    /// Forgets every history, keeping the frame count.
    pub(crate) fn forget(&mut self) {
        self.histories.clear();
    }

    /// Forgets the histories of the paths for which `path` returns false.
    pub(crate) fn keep(&mut self, mut path: impl FnMut(PathId) -> bool) {
        self.histories.retain(|id, _| path(*id));
    }

    /// Observes path `id` with the masks of its fill and its stroke in this frame, and reports
    /// whether it is drawn with them.
    ///
    /// The path's route depends on its masks:
    ///
    /// - If no part asks for a mask other than its most recent one, the path takes the route it
    ///   took when they last differed.
    /// - Otherwise the path takes masks, unless it is volatile and `path` has more than 4096
    ///   segments or its new masks would take the frame past 256 new masks or 2²⁰ texels of
    ///   volatile paths.
    ///
    /// A path without a history takes masks.
    ///
    /// The path changes in a frame when a part that kept a mask asks for one it did not keep. It
    /// is volatile when it changed in at least three of this frame and the three before, or when
    /// it was volatile in the frame before and changed in at least one of these four. A frame in
    /// which its masks do not differ is one without a change.
    pub(crate) fn route(&mut self, id: PathId, masks: [Option<&Mask>; 2], path: &BezPath) -> bool {
        let frame = self.frame;
        let history = self
            .histories
            .entry(id)
            .or_insert_with(|| History::new(frame));
        let differs = history
            .kept
            .iter()
            .zip(masks)
            .any(|(kept, mask)| mask.is_some_and(|mask| kept.first() != Some(mask)));
        if !differs {
            return !history.declined;
        }

        let before = if history.since == frame {
            history.volatile
        } else {
            history.volatile && history.changes_within(frame.saturating_sub(4), frame - 1) > 0
        };
        history.since = frame;

        let mut new = (0, 0);
        let mut changed = false;
        for (kept, mask) in history.kept.iter_mut().zip(masks) {
            let Some(mask) = mask else {
                continue;
            };
            if !kept.contains(mask) {
                changed |= !kept.is_empty();
                new = (new.0 + 1, new.1 + mask.texels());
            }
            kept.retain(|held| held != mask);
            kept.insert(0, mask.clone());
            kept.truncate(2);
        }

        if changed && history.changes.last() != Some(&frame) {
            history.changes.push(frame);
        }
        history
            .changes
            .retain(|change| frame - change < HISTORY_FRAMES);
        let recent = history.changes_within(frame.saturating_sub(3), frame);
        history.volatile = recent >= 3 || (before && recent >= 1);

        let (masks, texels) = (self.spent.0 + new.0, self.spent.1 + new.1);
        let over = masks > BUDGET_MASKS || texels > BUDGET_TEXELS;
        history.declined = history.volatile && (segments(path) > VOLATILE_SEGMENTS || over);
        if history.volatile && !history.declined {
            self.spent = (masks, texels);
        }
        !history.declined
    }
}

impl History {
    fn new(frame: u64) -> Self {
        Self {
            changes: Vec::new(),
            volatile: false,
            declined: false,
            since: frame,
            kept: [Vec::new(), Vec::new()],
        }
    }

    /// Returns how many frames from `first` to `last` the path changed in.
    fn changes_within(&self, first: u64, last: u64) -> usize {
        self.changes
            .iter()
            .filter(|frame| (first..=last).contains(*frame))
            .count()
    }
}

/// Returns the number of lines and curves of `path`.
fn segments(path: &BezPath) -> usize {
    path.elements()
        .iter()
        .filter(|element| !matches!(element, PathEl::MoveTo(_) | PathEl::ClosePath))
        .count()
}
