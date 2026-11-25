//! Compositor scrolling: routing gestures, distributing their deltas, and the physics of the
//! motion that follows.

mod advance;
mod distribute;
mod input;
pub(crate) mod motion;
pub(crate) mod over;
mod release;
pub(crate) mod route;
pub(crate) mod serials;
mod snap;
pub(crate) mod state;

use iceblink::{OutputId, ScrollId};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use self::state::ScrollState;

pub(crate) use self::advance::Change;
pub(crate) use self::input::Input;

/// The scroll nodes of one client that a gesture is latched onto, innermost first.
#[derive(Clone, Debug, Default)]
pub(crate) struct Latch {
    pub(crate) chain: SmallVec<[ScrollId; 4]>,
    /// Whether a finger holds the gesture, from its `Begin` to its `End`.
    pub(crate) finger: bool,
    /// The node that last absorbed a delta on each axis.
    pub(crate) moved: [Option<u32>; 2],
    /// When the gesture's last step happened.
    pub(crate) last_ns: u64,
}

/// The compositor's scroll state of one client's scroll nodes.
#[derive(Debug, Default)]
pub(crate) struct Scrolls {
    pub(crate) nodes: FxHashMap<u32, ScrollState>,
    /// The nodes the next frame advances: those in motion, and those owed an event.
    pub(crate) moving: SmallVec<[u32; 4]>,
    /// The gesture of each output that draws the client.
    pub(crate) latches: SmallVec<[(OutputId, Latch); 1]>,
}

impl Scrolls {
    /// Returns the state of node `id`, made at the offset `base` if the compositor has not
    /// moved the node before.
    pub(crate) fn state(&mut self, id: ScrollId, base: [f32; 2]) -> &mut ScrollState {
        let state = self
            .nodes
            .entry(id.slot())
            .or_insert_with(|| ScrollState::new(id, base));
        if state.id != id {
            *state = ScrollState::new(id, base);
        }
        state
    }

    /// Marks a node for the next frame to advance.
    pub(crate) fn wake(&mut self, slot: u32) {
        if !self.moving.contains(&slot) {
            self.moving.push(slot);
        }
    }

    /// Reports whether a later frame advances a node without further input: a node in motion or
    /// owed an event, unless a finger holds it still.
    pub(crate) fn advancing(&self) -> bool {
        self.moving.iter().any(|slot| {
            let held = self
                .latches
                .iter()
                .any(|(_, latch)| latch.finger && latch.chain.iter().any(|id| id.slot() == *slot));
            !held || self.nodes.get(slot).is_some_and(ScrollState::moves)
        })
    }

    /// Forgets a removed node.
    pub(crate) fn forget(&mut self, slot: u32) {
        self.nodes.remove(&slot);
        self.moving.retain(|moving| *moving != slot);
        for (_, latch) in &mut self.latches {
            latch.chain.retain(|id| id.slot() != slot);
        }
    }

    /// Returns the gesture of `output`, if one is latched.
    pub(crate) fn latch(&mut self, output: OutputId) -> Option<&mut Latch> {
        self.latches
            .iter_mut()
            .find(|(of, _)| *of == output)
            .map(|(_, latch)| latch)
    }

    /// Replaces the gesture of `output`.
    pub(crate) fn set_latch(&mut self, output: OutputId, latch: Option<Latch>) {
        self.latches.retain(|(of, _)| *of != output);
        if let Some(latch) = latch {
            self.latches.push((output, latch));
        }
    }
}
