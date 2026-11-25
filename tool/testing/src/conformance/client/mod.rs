//! The client suite: rules of the client seam, run on any stepped driver of any `Compositor`.
//!
//! Each rule is a public function that takes a [`Rig`] and panics when the driver it builds
//! breaks the rule. The rules drive a stepped driver through queued commits, gestures, watches
//! and steps at chosen clocks, and check every frame its renderer draws against the frame
//! contract.
//!
//! A case that hands gestures needs `CompositorCapabilities::scroll_input`, and [`run`],
//! [`run_all`] and [`client_suite!`](crate::client_suite) skip it on a compositor without it,
//! with a line on standard error.
//!
//! | Module | Rules on |
//! | --- | --- |
//! | [`commits`] | Commit answers and the frame that shows a commit |
//! | [`routes`] | Gestures routed otherwise than the client predicted |
//! | [`watches`] | Scroll watches, wakes and queued scroll events |
//! | [`lifecycle`] | Unregistering, snapshots, frame watches, verification, budgets and stepped runs |
//! | [`stepping`] | Client calls at one instant, and motion once per interval |
//! | [`creation`] | The first frame of an output, and renderers built after `create_output` returns |
//! | [`configure`] | The frame that shows a new configuration of an output |
//! | [`outputs`] | Failed renderers, renderer work and releases, frame observers, present and presenter watches, the display time |

pub mod commits;
pub mod configure;
pub mod creation;
mod flaky;
pub mod lifecycle;
mod noting;
pub mod outputs;
mod recorder;
pub mod routes;
mod seen;
mod stage;
pub mod stepping;
pub mod watches;

use iceblink::{Compositor, CompositorClient};

use crate::conformance::Case;
use crate::conformance::tier::Feature;

pub use self::lifecycle::transcript;
pub use self::recorder::{Log, Recorded, Release, SharedLog};
pub use self::stage::{ClientStage, STEP_NS};

/// The factories a case builds its driver from.
#[derive(Clone, Copy)]
pub struct Rig<'a> {
    /// Builds the compositor the driver runs.
    pub compositor: &'a dyn Fn() -> Box<dyn Compositor>,
    /// Wraps a compositor in a stepped driver.
    pub client: &'a dyn Fn(Box<dyn Compositor>) -> Box<dyn CompositorClient>,
}

/// A rule and the tier features a compositor needs for it.
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    /// The features the rule needs.
    pub needs: &'static [Feature],
    /// Checks the rule, panicking when the driver breaks it.
    pub check: fn(&Rig<'_>),
}

const SCROLL_INPUT: &[Feature] = &[Feature::ScrollInput];
const ANIMATIONS: &[Feature] = &[Feature::Animations];

/// Every case of the client suite, in the order [`run_all`] runs them.
pub const CASES: &[Case<Rule>] = &[
    case(
        "commits_are_answered_in_order_and_only_rejections_wake",
        &[],
        commits::commits_are_answered_in_order_and_only_rejections_wake,
    ),
    case(
        "a_commit_builds_one_frame_and_the_driver_idles",
        &[],
        commits::a_commit_builds_one_frame_and_the_driver_idles,
    ),
    case(
        "a_frame_shows_the_commits_queued_before_its_step",
        &[],
        commits::a_frame_shows_the_commits_queued_before_its_step,
    ),
    case(
        "a_gesture_routed_otherwise_than_predicted_comes_back",
        SCROLL_INPUT,
        routes::a_gesture_routed_otherwise_than_predicted_comes_back,
    ),
    case(
        "scrolling_wakes_no_client_without_a_watch",
        SCROLL_INPUT,
        watches::scrolling_wakes_no_client_without_a_watch,
    ),
    case(
        "a_queued_scroll_event_is_replaced_by_the_next_one_of_its_node",
        SCROLL_INPUT,
        watches::a_queued_scroll_event_is_replaced_by_the_next_one_of_its_node,
    ),
    case(
        "a_range_fires_once_when_the_offset_leaves_it",
        SCROLL_INPUT,
        watches::a_range_fires_once_when_the_offset_leaves_it,
    ),
    case(
        "a_range_checked_ahead_fires_before_the_offset_leaves_it",
        SCROLL_INPUT,
        watches::a_range_checked_ahead_fires_before_the_offset_leaves_it,
    ),
    case(
        "a_stride_fires_at_every_multiple_the_offset_passes",
        SCROLL_INPUT,
        watches::a_stride_fires_at_every_multiple_the_offset_passes,
    ),
    case(
        "a_watch_set_during_motion_knows_the_motion_step",
        SCROLL_INPUT,
        watches::a_watch_set_during_motion_knows_the_motion_step,
    ),
    case(
        "a_settle_watch_fires_when_the_node_rests",
        SCROLL_INPUT,
        watches::a_settle_watch_fires_when_the_node_rests,
    ),
    case(
        "an_unregistered_client_loses_its_content_and_its_queue",
        &[],
        lifecycle::an_unregistered_client_loses_its_content_and_its_queue,
    ),
    case(
        "snapshots_follow_the_frames_and_the_schedule",
        &[],
        lifecycle::snapshots_follow_the_frames_and_the_schedule,
    ),
    case(
        "a_frame_watch_reports_every_frame_drawn",
        &[],
        lifecycle::a_frame_watch_reports_every_frame_drawn,
    ),
    case(
        "verification_and_budgets_apply_in_queue_order",
        &[],
        lifecycle::verification_and_budgets_apply_in_queue_order,
    ),
    case(
        "an_output_builds_no_frame_before_a_commit",
        &[],
        creation::an_output_builds_no_frame_before_a_commit,
    ),
    case(
        "calls_queued_before_the_renderer_is_built_apply_after_it",
        &[],
        creation::calls_queued_before_the_renderer_is_built_apply_after_it,
    ),
    case(
        "a_new_size_is_drawn_only_with_the_commits_queued_after_it",
        &[],
        configure::a_new_size_is_drawn_only_with_the_commits_queued_after_it,
    ),
    case(
        "a_failed_renderer_reports_its_output",
        &[],
        outputs::a_failed_renderer_reports_its_output,
    ),
    case(
        "renderer_tasks_run_between_the_frames_around_them",
        &[],
        outputs::renderer_tasks_run_between_the_frames_around_them,
    ),
    case(
        "releases_reach_the_renderer",
        &[],
        outputs::releases_reach_the_renderer,
    ),
    case(
        "a_failed_acquisition_is_reported_and_retried",
        &[],
        outputs::a_failed_acquisition_is_reported_and_retried,
    ),
    case(
        "a_frame_observer_sees_each_frame_before_it_is_drawn",
        &[],
        outputs::a_frame_observer_sees_each_frame_before_it_is_drawn,
    ),
    case(
        "a_shown_watch_fires_once_on_the_next_present",
        &[],
        outputs::a_shown_watch_fires_once_on_the_next_present,
    ),
    case(
        "presenter_changes_follow_starved_and_occluded_frames",
        &[],
        outputs::presenter_changes_follow_starved_and_occluded_frames,
    ),
    case(
        "the_next_display_time_follows_the_schedule",
        &[],
        outputs::the_next_display_time_follows_the_schedule,
    ),
    case(
        "commits_at_one_instant_build_a_frame_each",
        &[],
        stepping::commits_at_one_instant_build_a_frame_each,
    ),
    case(
        "motion_advances_once_per_interval",
        ANIMATIONS,
        stepping::motion_advances_once_per_interval,
    ),
    case(
        "a_stepped_frame_is_built_at_its_step_clock",
        &[],
        stepping::a_stepped_frame_is_built_at_its_step_clock,
    ),
    case(
        "a_flooding_client_sees_each_frame_before_the_next",
        &[],
        stepping::a_flooding_client_sees_each_frame_before_the_next,
    ),
    case(
        "a_snapshot_is_current_once_it_counts_every_step",
        &[],
        stepping::a_snapshot_is_current_once_it_counts_every_step,
    ),
    case(
        "memory_is_current_after_a_sync",
        &[],
        lifecycle::memory_is_current_after_a_sync,
    ),
    case(
        "a_stepped_driver_reports_its_pacing",
        &[],
        lifecycle::a_stepped_driver_reports_its_pacing,
    ),
    case(
        "a_stepped_run_reproduces_its_frames_and_events",
        &[],
        lifecycle::a_stepped_run_reproduces_its_frames_and_events,
    ),
];

const fn case(name: &'static str, needs: &'static [Feature], check: fn(&Rig<'_>)) -> Case<Rule> {
    Case {
        name,
        run: Rule { needs, check },
    }
}

/// Runs the case named `name` on drivers from `rig`, or skips it with a line on standard error
/// when the compositor lacks a feature it needs.
///
/// # Panics
///
/// Panics when the suite has no such case, or when the driver breaks the case's rule.
pub fn run(name: &str, rig: &Rig<'_>) {
    let case = CASES
        .iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("the client suite has no case {name}"));
    run_case(case, rig);
}

/// Runs every case of the client suite on drivers from `rig`.
///
/// # Panics
///
/// Panics after the last case when any case failed, naming every failed case.
pub fn run_all(rig: &Rig<'_>) {
    crate::conformance::run_cases(CASES, |case| run_case(case, rig));
}

fn run_case(case: &Case<Rule>, rig: &Rig<'_>) {
    let caps = (rig.compositor)().capabilities();
    let missing: Vec<_> = case
        .run
        .needs
        .iter()
        .filter(|feature| !feature.reported(&caps))
        .collect();
    if !missing.is_empty() {
        eprintln!("skipped: {} needs {missing:?}", case.name);
        return;
    }
    (case.run.check)(rig);
}

/// Declares one test per case of the client suite, each on drivers from the rig expression
/// `$rig`, which is evaluated once per test.
///
/// ```text
/// iceblink_testing::client_suite!(iceblink_testing::conformance::client::Rig {
///     compositor: &|| -> Box<dyn iceblink::Compositor> { Box::new(MyCompositor::new()) },
///     client: &|compositor| -> Box<dyn iceblink::CompositorClient> {
///         Box::new(MyDriver::stepped(compositor))
///     },
/// });
/// ```
#[macro_export]
macro_rules! client_suite {
    ($rig:expr) => {
        $crate::client_suite!(
            @cases $rig;
            commits_are_answered_in_order_and_only_rejections_wake,
            a_commit_builds_one_frame_and_the_driver_idles,
            a_frame_shows_the_commits_queued_before_its_step,
            a_gesture_routed_otherwise_than_predicted_comes_back,
            scrolling_wakes_no_client_without_a_watch,
            a_queued_scroll_event_is_replaced_by_the_next_one_of_its_node,
            a_range_fires_once_when_the_offset_leaves_it,
            a_range_checked_ahead_fires_before_the_offset_leaves_it,
            a_stride_fires_at_every_multiple_the_offset_passes,
            a_watch_set_during_motion_knows_the_motion_step,
            a_settle_watch_fires_when_the_node_rests,
            an_unregistered_client_loses_its_content_and_its_queue,
            snapshots_follow_the_frames_and_the_schedule,
            a_frame_watch_reports_every_frame_drawn,
            verification_and_budgets_apply_in_queue_order,
            an_output_builds_no_frame_before_a_commit,
            calls_queued_before_the_renderer_is_built_apply_after_it,
            a_new_size_is_drawn_only_with_the_commits_queued_after_it,
            a_failed_renderer_reports_its_output,
            renderer_tasks_run_between_the_frames_around_them,
            releases_reach_the_renderer,
            a_failed_acquisition_is_reported_and_retried,
            a_frame_observer_sees_each_frame_before_it_is_drawn,
            a_shown_watch_fires_once_on_the_next_present,
            presenter_changes_follow_starved_and_occluded_frames,
            the_next_display_time_follows_the_schedule,
            commits_at_one_instant_build_a_frame_each,
            motion_advances_once_per_interval,
            a_stepped_frame_is_built_at_its_step_clock,
            a_flooding_client_sees_each_frame_before_the_next,
            a_snapshot_is_current_once_it_counts_every_step,
            memory_is_current_after_a_sync,
            a_stepped_driver_reports_its_pacing,
            a_stepped_run_reproduces_its_frames_and_events
        );
    };
    (@cases $rig:expr; $($case:ident),+ $(,)?) => {
        $(
            #[test]
            fn $case() {
                $crate::conformance::client::run(stringify!($case), &$rig);
            }
        )+
    };
}
