//! Rules of compositor scrolling: routing, delta distribution, physics, snapping, overscroll and
//! reconciliation with the client.

mod latching;
mod physics;
mod reconciliation;

pub use self::latching::{
    a_gesture_latches_onto_the_innermost_node_that_can_move,
    an_unlatched_release_routes_by_its_velocity, gestures_need_a_node_that_scrolls,
    the_rest_of_a_delta_chains_outwards, the_rest_past_the_end_clamps_or_bounces,
};
pub use self::physics::{
    a_finger_holding_a_node_still_lets_the_output_sleep, a_fling_decays_by_the_formula,
    a_kinetic_step_follows_its_curve, a_pull_past_the_end_returns,
    instant_deltas_move_the_node_by_the_next_frame, motion_snaps_to_the_nearest_snap_point,
    returns_and_snaps_settle_in_the_frame_that_lands_them, steps_move_the_node_in_the_step_phase,
};
pub use self::reconciliation::{
    a_gesture_ends_an_offset_animation, a_held_offset_animation_lets_the_output_sleep,
    a_node_write_at_the_base_offset_keeps_the_motion, acknowledging_a_serial_moves_no_pixel,
    an_offset_write_ends_the_motion, serials_increase_per_event,
};
