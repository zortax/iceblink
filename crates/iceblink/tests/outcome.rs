//! Which frame outcomes retire damage and ask for another frame.

use iceblink::{FrameOutcome, FrameStats, SkipReason};

const SKIPS: [SkipReason; 10] = [
    SkipReason::Unconfigured,
    SkipReason::Timeout,
    SkipReason::Declined,
    SkipReason::Unacquired,
    SkipReason::Occluded,
    SkipReason::Outdated,
    SkipReason::Validation,
    SkipReason::DeviceUnavailable,
    SkipReason::Undamaged,
    SkipReason::Held,
];

#[test]
fn only_outcomes_that_recorded_nothing_keep_their_damage() {
    assert!(FrameOutcome::Presented(FrameStats::default()).retires_damage());
    assert!(FrameOutcome::Recovered.retires_damage());
    for reason in SKIPS {
        let keeps = matches!(
            reason,
            SkipReason::Unconfigured | SkipReason::Unacquired | SkipReason::DeviceUnavailable
        );
        assert_eq!(
            FrameOutcome::Skipped(reason).retires_damage(),
            !keeps,
            "{reason:?}"
        );
    }
}

#[test]
fn outcomes_that_wait_for_the_platform_ask_for_no_other_frame() {
    assert!(!FrameOutcome::Presented(FrameStats::default()).wants_another_frame());
    assert!(FrameOutcome::Recovered.wants_another_frame());
    for reason in SKIPS {
        let waits = matches!(
            reason,
            SkipReason::Occluded
                | SkipReason::Undamaged
                | SkipReason::Held
                | SkipReason::DeviceUnavailable
        );
        assert_eq!(
            FrameOutcome::Skipped(reason).wants_another_frame(),
            !waits,
            "{reason:?}"
        );
    }
}
