//! The present damage of recent frames, for images that hold older frames.

use std::collections::VecDeque;

use iceblink::damage::DamageSet;

/// How many presented frames are remembered.
const DEPTH: usize = 8;

/// The `present` damage of the most recent frames of one output, newest first.
#[derive(Debug, Default)]
pub(crate) struct PresentHistory {
    frames: VecDeque<DamageSet>,
}

impl PresentHistory {
    /// Remembers the present damage of a frame that was just drawn.
    pub(crate) fn push(&mut self, present: DamageSet) {
        if self.frames.len() == DEPTH {
            self.frames.pop_back();
        }
        self.frames.push_front(present);
    }

    /// Returns the union of the present damage of the `count` most recent frames, or `None`
    /// when fewer are remembered.
    pub(crate) fn union(&self, count: usize) -> Option<DamageSet> {
        if count > self.frames.len() {
            return None;
        }

        let mut union = DamageSet::new();
        for frame in self.frames.iter().take(count) {
            union.absorb_set(frame);
        }
        Some(union)
    }

    /// Forgets every remembered frame.
    pub(crate) fn clear(&mut self) {
        self.frames.clear();
    }
}
