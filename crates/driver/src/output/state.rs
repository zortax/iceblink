//! Whether an output's presenter is starved or hidden.

use iceblink::{FrameOutcome, SkipReason};

/// What an output's recent frames say of its presenter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PresenterState {
    /// Frames timed out waiting for an image since the last frame presented.
    pub(crate) starved: bool,
    /// The platform reported the output hidden since the last frame presented.
    pub(crate) occluded: bool,
}

impl PresenterState {
    /// Takes the outcome of a frame into the state.
    pub(crate) fn observe(&mut self, outcome: FrameOutcome) {
        match outcome {
            FrameOutcome::Presented(_) => *self = Self::default(),
            FrameOutcome::Skipped(SkipReason::Timeout) => self.starved = true,
            FrameOutcome::Skipped(SkipReason::Occluded) => self.occluded = true,
            _ => {}
        }
    }
}
