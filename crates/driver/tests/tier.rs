//! The tier conformance cases that hold without frames at chosen times, through a client handle.
//!
//! The other cases build frames at present times they choose, which the driver takes from its
//! presenters.

mod common;

use iceblink::Compositor;

use crate::common::thread::remote::Remote;

fn factory() -> Box<dyn Compositor> {
    Box::new(Remote::new())
}

iceblink_testing::tier_suite!(
    @cases factory;
    malformed_animations_are_rejected,
    a_gesture_latches_onto_the_innermost_node_that_can_move,
    gestures_need_a_node_that_scrolls,
    a_blank_part_of_a_port_routes_to_the_port,
    a_port_whose_content_draws_nothing_routes_to_the_port,
    what_is_drawn_over_a_port_keeps_its_gestures,
    a_blank_part_of_a_nested_port_routes_to_the_inner_port,
    a_port_counts_only_inside_the_ports_around_it,
    hits_come_front_to_back,
    rounded_corners_are_hit_exactly,
    clips_cut_hits,
    soft_clip_links_hit_nothing_outside_their_box,
    the_local_point_undoes_the_transform_and_the_offset,
    paths_are_hit_by_their_fill_rule,
    shadows_and_transparent_chunks_are_never_hit,
    clip_links_strokes_and_frames_are_hit_exactly,
);
