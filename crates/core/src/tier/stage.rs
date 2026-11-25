//! A client commit's changes to the tier, held until the commit succeeds.

use iceblink::ScrollId;
use smallvec::SmallVec;

/// A scroll node's base offset and compositor delta as the commands applied so far leave them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StagedScroll {
    pub(crate) id: ScrollId,
    pub(crate) base: [f32; 2],
    pub(crate) delta: [f64; 2],
    /// Whether an offset write made the delta zero, which ends the node's motion.
    pub(crate) reset: bool,
    /// The newest serial acknowledged.
    pub(crate) acknowledged: Option<u64>,
    /// The delta at that serial, which acknowledging it took out of the delta.
    pub(crate) adopted: [f64; 2],
}

/// The tier changes of a client commit, applied once it succeeds.
#[derive(Debug, Default)]
pub(crate) struct TierStage {
    pub(crate) scrolls: SmallVec<[StagedScroll; 2]>,
    /// The positions of the commands that change the tier once the commit succeeds, in order.
    pub(crate) commands: SmallVec<[u32; 4]>,
    /// Whether the commit starts an animation.
    pub(crate) animates: bool,
}

impl TierStage {
    /// Returns the staged state of node `id`, if the commit changed it.
    pub(crate) fn scroll(&self, id: ScrollId) -> Option<StagedScroll> {
        self.scrolls.iter().copied().find(|staged| staged.id == id)
    }

    /// Stages the state of a node, replacing what was staged for it.
    pub(crate) fn set_scroll(&mut self, staged: StagedScroll) {
        match self.scrolls.iter_mut().find(|held| held.id == staged.id) {
            Some(held) => *held = staged,
            None => self.scrolls.push(staged),
        }
    }
}
