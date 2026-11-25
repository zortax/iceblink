//! The tier suite: rules of the compositor-driven tier, checked on any `Compositor` at chosen
//! present times.
//!
//! Each rule is a public function that takes a [`Factory`] and panics when the compositor it
//! builds breaks the rule. The rules drive the compositor through commits, frames at chosen
//! present times, gestures, hit tests and events, and check every frame against the frame
//! contract.
//!
//! Each case declares the tier features it needs. [`run`], [`run_all`] and
//! [`tier_suite!`](crate::tier_suite) skip a case whose features the compositor does not report,
//! with a line on standard error.
//!
//! | Module | Rules on |
//! | --- | --- |
//! | [`animations`] | Sampling, easing, writes, replacement, cancellation, events and waking |
//! | [`routing`] | Routing gestures by the port under the position |
//! | [`sources`] | Overscroll and chaining per source of gestures |
//! | [`scrolling`] | Routing, delta distribution, physics, snapping, overscroll and reconciliation |
//! | [`adoption`] | Acknowledgements of scrolling shown at whole device pixels |
//! | [`pending`] | Gesture steps that wait for the next frame while the client commits |
//! | [`hits`] | Hit testing through shapes, clips, transforms and order |
//! | [`thumbs`] | Thumb nodes that follow compositor scrolling |

pub mod adoption;
pub mod animations;
pub mod hits;
pub mod pending;
pub mod routing;
mod scrollers;
pub mod scrolling;
pub mod sources;
mod stage;
pub mod thumbs;

use iceblink::CompositorCapabilities;

use crate::conformance::Case;

pub use self::stage::{FRAME_NS, Factory, Stage};

/// A feature of the compositor-driven tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// Compositor-driven animations.
    Animations,
    /// Gestures handed to the compositor.
    ScrollInput,
    /// Hit testing.
    HitTest,
}

impl Feature {
    /// Reports whether `caps` holds the feature.
    pub fn reported(self, caps: &CompositorCapabilities) -> bool {
        match self {
            Self::Animations => caps.animations,
            Self::ScrollInput => caps.scroll_input,
            Self::HitTest => caps.hit_test,
        }
    }
}

const ANIMATIONS: Feature = Feature::Animations;
const SCROLL_INPUT: Feature = Feature::ScrollInput;
const HIT_TEST: Feature = Feature::HitTest;

/// A rule and the features a compositor needs for it.
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    /// The features the rule needs.
    pub needs: &'static [Feature],
    /// Checks the rule, panicking when the compositor breaks it.
    pub check: fn(Factory<'_>),
}

/// Every case of the tier suite, in the order [`run_all`] runs them.
pub const CASES: &[Case<Rule>] = &[
    case(
        "an_animation_is_sampled_at_the_present_time",
        &[ANIMATIONS],
        animations::an_animation_is_sampled_at_the_present_time,
    ),
    case(
        "timing_and_keyframe_easings_both_apply",
        &[ANIMATIONS],
        animations::timing_and_keyframe_easings_both_apply,
    ),
    case(
        "a_transform_animates_by_decomposition",
        &[ANIMATIONS],
        animations::a_transform_animates_by_decomposition,
    ),
    case(
        "a_filter_parameter_animates",
        &[ANIMATIONS],
        animations::a_filter_parameter_animates,
    ),
    case(
        "an_explicit_write_replaces_the_animation",
        &[ANIMATIONS],
        animations::an_explicit_write_replaces_the_animation,
    ),
    case(
        "a_new_animation_replaces_the_running_one",
        &[ANIMATIONS],
        animations::a_new_animation_replaces_the_running_one,
    ),
    case(
        "cancelling_returns_the_base_value",
        &[ANIMATIONS],
        animations::cancelling_returns_the_base_value,
    ),
    case(
        "the_fill_decides_the_value_after_the_end",
        &[ANIMATIONS],
        animations::the_fill_decides_the_value_after_the_end,
    ),
    case(
        "removing_the_target_cancels_its_animations",
        &[ANIMATIONS],
        animations::removing_the_target_cancels_its_animations,
    ),
    case(
        "events_follow_the_life_of_an_animation",
        &[ANIMATIONS],
        animations::events_follow_the_life_of_an_animation,
    ),
    case(
        "a_delayed_animation_wakes_an_idle_output",
        &[ANIMATIONS],
        animations::a_delayed_animation_wakes_an_idle_output,
    ),
    case(
        "a_sample_damages_what_the_equal_write_damages",
        &[ANIMATIONS],
        animations::a_sample_damages_what_the_equal_write_damages,
    ),
    case(
        "malformed_animations_are_rejected",
        &[ANIMATIONS],
        animations::malformed_animations_are_rejected,
    ),
    case(
        "cancelling_an_ended_animation_does_nothing",
        &[ANIMATIONS],
        animations::cancelling_an_ended_animation_does_nothing,
    ),
    case(
        "a_gesture_latches_onto_the_innermost_node_that_can_move",
        &[SCROLL_INPUT],
        scrolling::a_gesture_latches_onto_the_innermost_node_that_can_move,
    ),
    case(
        "the_rest_of_a_delta_chains_outwards",
        &[SCROLL_INPUT],
        scrolling::the_rest_of_a_delta_chains_outwards,
    ),
    case(
        "the_rest_past_the_end_clamps_or_bounces",
        &[SCROLL_INPUT],
        scrolling::the_rest_past_the_end_clamps_or_bounces,
    ),
    case(
        "instant_deltas_move_the_node_by_the_next_frame",
        &[SCROLL_INPUT],
        scrolling::instant_deltas_move_the_node_by_the_next_frame,
    ),
    case(
        "a_fling_decays_by_the_formula",
        &[SCROLL_INPUT],
        scrolling::a_fling_decays_by_the_formula,
    ),
    case(
        "a_kinetic_step_follows_its_curve",
        &[SCROLL_INPUT],
        scrolling::a_kinetic_step_follows_its_curve,
    ),
    case(
        "steps_move_the_node_in_the_step_phase",
        &[SCROLL_INPUT],
        scrolling::steps_move_the_node_in_the_step_phase,
    ),
    case(
        "motion_snaps_to_the_nearest_snap_point",
        &[SCROLL_INPUT],
        scrolling::motion_snaps_to_the_nearest_snap_point,
    ),
    case(
        "a_finger_holding_a_node_still_lets_the_output_sleep",
        &[SCROLL_INPUT],
        scrolling::a_finger_holding_a_node_still_lets_the_output_sleep,
    ),
    case(
        "a_pull_past_the_end_returns",
        &[SCROLL_INPUT],
        scrolling::a_pull_past_the_end_returns,
    ),
    case(
        "returns_and_snaps_settle_in_the_frame_that_lands_them",
        &[SCROLL_INPUT],
        scrolling::returns_and_snaps_settle_in_the_frame_that_lands_them,
    ),
    case(
        "acknowledging_a_serial_moves_no_pixel",
        &[SCROLL_INPUT],
        scrolling::acknowledging_a_serial_moves_no_pixel,
    ),
    case(
        "acknowledging_a_half_pixel_moves_no_pixel",
        &[SCROLL_INPUT],
        adoption::acknowledging_a_half_pixel_moves_no_pixel,
    ),
    case(
        "an_offset_write_ends_the_motion",
        &[ANIMATIONS, SCROLL_INPUT],
        scrolling::an_offset_write_ends_the_motion,
    ),
    case(
        "a_node_write_at_the_base_offset_keeps_the_motion",
        &[SCROLL_INPUT],
        scrolling::a_node_write_at_the_base_offset_keeps_the_motion,
    ),
    case(
        "a_gesture_ends_an_offset_animation",
        &[ANIMATIONS, SCROLL_INPUT],
        scrolling::a_gesture_ends_an_offset_animation,
    ),
    case(
        "gestures_need_a_node_that_scrolls",
        &[SCROLL_INPUT],
        scrolling::gestures_need_a_node_that_scrolls,
    ),
    case(
        "serials_increase_per_event",
        &[SCROLL_INPUT],
        scrolling::serials_increase_per_event,
    ),
    case(
        "an_unlatched_release_routes_by_its_velocity",
        &[SCROLL_INPUT],
        scrolling::an_unlatched_release_routes_by_its_velocity,
    ),
    case(
        "a_held_offset_animation_lets_the_output_sleep",
        &[ANIMATIONS, SCROLL_INPUT],
        scrolling::a_held_offset_animation_lets_the_output_sleep,
    ),
    case(
        "an_update_waiting_for_an_adoption_shows_with_its_event",
        &[SCROLL_INPUT],
        pending::an_update_waiting_for_an_adoption_shows_with_its_event,
    ),
    case(
        "a_node_write_waiting_for_its_frame_keeps_the_shown_offset",
        &[SCROLL_INPUT, HIT_TEST],
        pending::a_node_write_waiting_for_its_frame_keeps_the_shown_offset,
    ),
    case(
        "overscroll_bounces_only_the_sources_that_bounce",
        &[SCROLL_INPUT],
        sources::overscroll_bounces_only_the_sources_that_bounce,
    ),
    case(
        "the_rest_chains_only_for_the_sources_that_chain",
        &[SCROLL_INPUT],
        sources::the_rest_chains_only_for_the_sources_that_chain,
    ),
    case(
        "a_blank_part_of_a_port_routes_to_the_port",
        &[SCROLL_INPUT],
        routing::a_blank_part_of_a_port_routes_to_the_port,
    ),
    case(
        "a_port_whose_content_draws_nothing_routes_to_the_port",
        &[SCROLL_INPUT],
        routing::a_port_whose_content_draws_nothing_routes_to_the_port,
    ),
    case(
        "what_is_drawn_over_a_port_keeps_its_gestures",
        &[SCROLL_INPUT],
        routing::what_is_drawn_over_a_port_keeps_its_gestures,
    ),
    case(
        "a_blank_part_of_a_nested_port_routes_to_the_inner_port",
        &[SCROLL_INPUT],
        routing::a_blank_part_of_a_nested_port_routes_to_the_inner_port,
    ),
    case(
        "a_port_counts_only_inside_the_ports_around_it",
        &[SCROLL_INPUT],
        routing::a_port_counts_only_inside_the_ports_around_it,
    ),
    case(
        "a_thumb_follows_compositor_scrolling",
        &[SCROLL_INPUT, HIT_TEST],
        thumbs::a_thumb_follows_compositor_scrolling,
    ),
    case(
        "hits_come_front_to_back",
        &[HIT_TEST],
        hits::hits_come_front_to_back,
    ),
    case(
        "rounded_corners_are_hit_exactly",
        &[HIT_TEST],
        hits::rounded_corners_are_hit_exactly,
    ),
    case("clips_cut_hits", &[HIT_TEST], hits::clips_cut_hits),
    case(
        "soft_clip_links_hit_nothing_outside_their_box",
        &[HIT_TEST],
        hits::soft_clip_links_hit_nothing_outside_their_box,
    ),
    case(
        "the_local_point_undoes_the_transform_and_the_offset",
        &[HIT_TEST],
        hits::the_local_point_undoes_the_transform_and_the_offset,
    ),
    case(
        "paths_are_hit_by_their_fill_rule",
        &[HIT_TEST],
        hits::paths_are_hit_by_their_fill_rule,
    ),
    case(
        "shadows_and_transparent_chunks_are_never_hit",
        &[HIT_TEST],
        hits::shadows_and_transparent_chunks_are_never_hit,
    ),
    case(
        "clip_links_strokes_and_frames_are_hit_exactly",
        &[HIT_TEST],
        hits::clip_links_strokes_and_frames_are_hit_exactly,
    ),
];

const fn case(name: &'static str, needs: &'static [Feature], check: fn(Factory<'_>)) -> Case<Rule> {
    Case {
        name,
        run: Rule { needs, check },
    }
}

/// Runs `case` on compositors from `factory`, or skips it when the compositor lacks a feature it
/// needs.
fn run_case(case: &Case<Rule>, factory: Factory<'_>) {
    let caps = factory().capabilities();
    if let Some(missing) = case.run.needs.iter().find(|need| !need.reported(&caps)) {
        eprintln!(
            "skipped: tier case {}: the compositor does not report {missing:?}",
            case.name
        );
        return;
    }
    (case.run.check)(factory);
}

/// Runs the case named `name` on compositors from `factory`, or skips it when the compositor lacks
/// a feature it needs.
///
/// # Panics
///
/// Panics when no case has the name, and when the compositor breaks the rule.
#[track_caller]
pub fn run(name: &str, factory: Factory<'_>) {
    let case = CASES
        .iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("the tier suite has no case {name}"));
    run_case(case, factory);
}

/// Runs every case of the tier suite on compositors from `factory`.
///
/// # Panics
///
/// Panics after the last case when any case failed, naming every failed case.
pub fn run_all(factory: Factory<'_>) {
    crate::conformance::run_cases(CASES, |case| run_case(case, factory));
}

/// Declares one test per case of the tier suite, each on compositors from the factory expression
/// `$make`, which is evaluated once per test.
///
/// ```text
/// iceblink_testing::tier_suite!(|| -> Box<dyn iceblink::Compositor> {
///     Box::new(MyCompositor::new())
/// });
/// ```
///
/// The form `tier_suite!(@cases $make; case, ...)` declares tests for the named cases only, for a
/// compositor that runs only some of them.
#[macro_export]
macro_rules! tier_suite {
    ($make:expr) => {
        $crate::tier_suite!(
            @cases $make;
            an_animation_is_sampled_at_the_present_time, timing_and_keyframe_easings_both_apply,
            a_transform_animates_by_decomposition, a_filter_parameter_animates,
            an_explicit_write_replaces_the_animation, a_new_animation_replaces_the_running_one,
            cancelling_returns_the_base_value, the_fill_decides_the_value_after_the_end,
            removing_the_target_cancels_its_animations, events_follow_the_life_of_an_animation,
            a_delayed_animation_wakes_an_idle_output, a_sample_damages_what_the_equal_write_damages,
            malformed_animations_are_rejected, cancelling_an_ended_animation_does_nothing,
            a_gesture_latches_onto_the_innermost_node_that_can_move,
            the_rest_of_a_delta_chains_outwards, the_rest_past_the_end_clamps_or_bounces,
            instant_deltas_move_the_node_by_the_next_frame, a_fling_decays_by_the_formula,
            a_kinetic_step_follows_its_curve, steps_move_the_node_in_the_step_phase,
            motion_snaps_to_the_nearest_snap_point,
            a_finger_holding_a_node_still_lets_the_output_sleep, a_pull_past_the_end_returns,
            returns_and_snaps_settle_in_the_frame_that_lands_them,
            acknowledging_a_serial_moves_no_pixel, acknowledging_a_half_pixel_moves_no_pixel,
            an_offset_write_ends_the_motion, a_node_write_at_the_base_offset_keeps_the_motion,
            a_gesture_ends_an_offset_animation, gestures_need_a_node_that_scrolls,
            serials_increase_per_event, an_unlatched_release_routes_by_its_velocity,
            a_held_offset_animation_lets_the_output_sleep,
            an_update_waiting_for_an_adoption_shows_with_its_event,
            a_node_write_waiting_for_its_frame_keeps_the_shown_offset,
            overscroll_bounces_only_the_sources_that_bounce,
            the_rest_chains_only_for_the_sources_that_chain,
            a_blank_part_of_a_port_routes_to_the_port,
            a_port_whose_content_draws_nothing_routes_to_the_port,
            what_is_drawn_over_a_port_keeps_its_gestures,
            a_blank_part_of_a_nested_port_routes_to_the_inner_port,
            a_port_counts_only_inside_the_ports_around_it, a_thumb_follows_compositor_scrolling,
            hits_come_front_to_back, rounded_corners_are_hit_exactly, clips_cut_hits,
            soft_clip_links_hit_nothing_outside_their_box,
            the_local_point_undoes_the_transform_and_the_offset, paths_are_hit_by_their_fill_rule,
            shadows_and_transparent_chunks_are_never_hit,
            clip_links_strokes_and_frames_are_hit_exactly
        );
    };
    (@cases $make:expr; $($case:ident),+ $(,)?) => {
        $(
            #[test]
            fn $case() {
                $crate::conformance::tier::run(stringify!($case), &$make);
            }
        )+
    };
}
