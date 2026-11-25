//! The outcome of a frame from what presenting it answered.

use iceblink::{FrameOutcome, FrameStats, PresentResult, SkipReason};

/// Returns the outcome of a frame that was composed and then presented with `result`.
pub(crate) fn of_present(result: PresentResult, stats: FrameStats) -> FrameOutcome {
    match result {
        PresentResult::Presented => FrameOutcome::Presented(stats),
        PresentResult::Held => FrameOutcome::Skipped(SkipReason::Held),
        PresentResult::Skipped(reason) => FrameOutcome::Skipped(retiring(reason)),
    }
}

/// Returns the skip reason of a frame that was already composed and then skipped for `reason`.
///
/// The returned reason retires the frame's damage. The reasons that keep damage, `Unconfigured`,
/// `Unacquired` and `DeviceUnavailable`, become `Timeout`, and every other reason is kept.
// The reasons that keep damage say nothing was recorded, which is no longer true.
pub(crate) fn retiring(reason: SkipReason) -> SkipReason {
    match reason {
        SkipReason::Unconfigured | SkipReason::Unacquired | SkipReason::DeviceUnavailable => {
            SkipReason::Timeout
        }
        other => other,
    }
}
