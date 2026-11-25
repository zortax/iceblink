//! The counters a compositor keeps of its most recent commit and frame.

mod commit;
mod frame;

pub use self::commit::CommitCounters;
pub use self::frame::FrameCounters;

/// The counters a compositor keeps.
///
/// They describe the most recent commit and the most recent frame. A compositor that does not
/// track a counter reports zero for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    /// The most recent commit.
    pub commit: CommitCounters,
    /// The most recent frame.
    pub frame: FrameCounters,
    /// Hint violations found by verification since the compositor was created.
    pub hint_violations: u64,
    /// Declared damage found by verification to miss derived damage, since the compositor was
    /// created.
    pub damage_violations: u64,
}
