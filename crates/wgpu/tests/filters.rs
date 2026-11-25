//! Filter chains on groups and backdrops: blurs, drop shadows and colour matrices.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, frame_filter, frosted, group, quad, rect};
use common::device::{plain_renderer, present};
use iceblink::{Filter, Frame, read_extent};
use iceblink_wgpu::GroupPrecision;

/// Returns a black square of `side` in the middle of a white output, moved by `offset` and
/// blurred by `deviation`.
fn blurred_square(deviation: f32, offset: [f32; 2], side: f32) -> Frame {
    let mut builder = FrameBuilder::new();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    let at = (128.0 - side) * 0.5;
    let bounds = rect(at + offset[0], at + offset[1], side, side);
    builder.begin_group(group(bounds, 1.0, &[Filter::Blur(deviation)]));
    quad(&mut builder, bounds, [0, 0, 0]);
    builder.end_group();
    builder.finish()
}

// The blur runs on gamma-encoded values, as CSS specifies: the two sides of an edge sum to white,
// where a blur in linear light would sum to about 372.
#[test]
fn a_blurred_edge_crosses_mid_grey_where_css_says() {
    let mut renderer = plain_renderer();
    for deviation in [4.0f32, 8.0] {
        let pixels = present(&mut renderer, &blurred_square(deviation, [0.0; 2], 64.0));
        for step in [1i32, 3, 5] {
            let outside = i32::from(pixels.rgba(31 - step, 64)[0]);
            let inside = i32::from(pixels.rgba(32 + step, 64)[0]);
            assert!(
                (outside + inside - 255).abs() <= 12,
                "deviation {deviation}, {step} px out: {outside} and {inside}"
            );
        }
        let edge = i32::from(pixels.rgba(32, 64)[0]);
        assert!(
            (edge - 188).abs() > 40,
            "deviation {deviation}: the edge reads {edge}, the linear-light answer"
        );
    }
}

#[test]
fn a_blur_reaches_three_deviations_and_keeps_the_middle_solid() {
    let mut renderer = plain_renderer();

    let pixels = present(&mut renderer, &blurred_square(6.0, [0.0; 2], 64.0));

    let inside_reach = pixels.rgba(24, 64)[0];
    let outside_reach = pixels.rgba(2, 64)[0];
    assert!(inside_reach < 240, "eight pixels out: {inside_reach}");
    assert!(outside_reach >= 253, "thirty pixels out: {outside_reach}");
    assert!(pixels.rgba(64, 64)[0] <= 4, "the middle stays black");
}

/// Returns where `profile` first falls through `level`, in fractional samples.
fn crossing(profile: &[f32], level: f32) -> f32 {
    for (index, pair) in profile.windows(2).enumerate() {
        let (high, low) = (pair[0], pair[1]);
        if high >= level && low <= level && high > low {
            return index as f32 + (high - level) / (high - low);
        }
    }
    panic!("the profile never crosses {level}: {profile:?}");
}

#[test]
fn a_blurred_group_at_sub_pixel_offsets_moves_its_halo_by_the_offset() {
    let mut renderer = plain_renderer();
    let mut crossings = Vec::new();
    for step in 0..5 {
        let offset = step as f32 * 0.25;
        let pixels = present(&mut renderer, &blurred_square(6.0, [offset, 0.0], 64.0));
        let profile: Vec<f32> = (16..48).map(|x| f32::from(pixels.rgba(x, 64)[0])).collect();
        crossings.push(crossing(&profile, 128.0) + 16.0);
    }

    for (step, crossed) in crossings.iter().enumerate() {
        let expected = crossings[0] + step as f32 * 0.25;
        assert!(
            (crossed - expected).abs() <= 0.3,
            "at a {} px offset the halo is at {crossed:.2}, not {expected:.2}",
            step as f32 * 0.25
        );
    }
    assert!(crossings[4] - crossings[0] > 0.6, "a whole pixel moves it");
}

/// Returns a colour matrix that scales contrast by `amount` about mid grey.
fn contrast(amount: f32) -> Filter {
    let offset = 0.5 * (1.0 - amount);
    Filter::ColorMatrix([
        amount, 0.0, 0.0, 0.0, offset, 0.0, amount, 0.0, 0.0, offset, 0.0, 0.0, amount, 0.0,
        offset, 0.0, 0.0, 0.0, 1.0, 0.0,
    ])
}

#[test]
fn a_colour_matrix_before_a_blur_draws_another_picture_than_after_it() {
    let mut renderer = plain_renderer();
    let mut drawn = |filters: &[Filter]| {
        let mut builder = FrameBuilder::new();
        quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
        builder.begin_group(group(rect(32.0, 32.0, 64.0, 64.0), 1.0, filters));
        quad(&mut builder, rect(32.0, 32.0, 32.0, 64.0), [26, 26, 26]);
        quad(&mut builder, rect(64.0, 32.0, 32.0, 64.0), [128, 128, 128]);
        builder.end_group();
        present(&mut renderer, &builder.finish())
    };

    let before = drawn(&[contrast(4.0), Filter::Blur(5.0)]);
    let after = drawn(&[Filter::Blur(5.0), contrast(4.0)]);

    assert!(
        before.max_difference(&after) > 8,
        "the two orders draw the same picture"
    );
    assert!(
        renderer.groups().peak() >= 2,
        "a matrix before a blur needs a target of its own"
    );
}

/// Returns a red left half and a black square under a backdrop filtered by `backdrop`, inside a
/// group of the whole output when `grouped`.
fn edge_frame(backdrop: Option<Filter>, grouped: bool) -> Frame {
    let mut builder = FrameBuilder::new();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    if grouped {
        builder.begin_group(group(rect(0.0, 0.0, 128.0, 128.0), 1.0, &[]));
    }
    quad(&mut builder, rect(0.0, 0.0, 64.0, 128.0), [255, 0, 0]);
    quad(&mut builder, rect(40.0, 40.0, 16.0, 16.0), [0, 0, 0]);
    if let Some(filter) = backdrop {
        let bounds = rect(32.0, 32.0, 64.0, 64.0);
        let mut frosted = frosted(bounds, 1.0);
        frosted.source = read_extent(bounds, &[filter]);
        frosted.filters = Arc::from(vec![frame_filter(&filter)]);
        builder.backdrop(frosted);
    }
    if grouped {
        builder.end_group();
    }
    builder.finish()
}

/// Returns the bytes the pool holds after drawing `frame` with the default budget.
fn pool_bytes(frame: &Frame, precision: GroupPrecision) -> u64 {
    let mut renderer = plain_renderer();
    renderer.set_group_precision(precision);
    present(&mut renderer, frame);
    renderer.groups().bytes()
}

#[test]
fn a_backdrop_the_budget_cannot_copy_at_full_resolution_is_left_unfiltered() {
    let frame = edge_frame(Some(Filter::Blur(4.0)), false);
    let needed = pool_bytes(&frame, GroupPrecision::Compact);

    let mut renderer = plain_renderer();
    renderer.set_group_budget(needed - 1);

    let frosted = present(&mut renderer, &frame);

    assert_eq!(renderer.groups().degraded(), 0, "no lease was reduced");
    let plain = present(&mut renderer, &edge_frame(None, false));
    assert_eq!(
        frosted.max_difference(&plain),
        0,
        "the backdrop drew nothing, so the frame is the content beneath it"
    );
}

#[test]
fn a_backdrop_in_a_half_resolution_group_filters_what_is_beneath_it_in_place() {
    let one = pool_bytes(&edge_frame(None, true), GroupPrecision::High);
    let mut renderer = plain_renderer();
    renderer.set_group_precision(GroupPrecision::High);
    renderer.set_group_budget(3 * one / 4);
    let grey = Filter::ColorMatrix([
        0.2126, 0.7152, 0.0722, 0.0, 0.0, 0.2126, 0.7152, 0.0722, 0.0, 0.0, 0.2126, 0.7152, 0.0722,
        0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
    ]);

    let filtered = present(&mut renderer, &edge_frame(Some(grey), true));

    assert!(
        renderer.groups().degraded() >= 1,
        "the group is at half resolution"
    );
    let plain = present(&mut renderer, &edge_frame(None, true));
    let red = filtered.rgba(36, 80);
    assert!(
        red[0].abs_diff(red[1]) <= 4
            && red[1].abs_diff(red[2]) <= 4
            && (30..=110).contains(&red[0]),
        "the red half under the backdrop is grey: {red:?}"
    );
    assert!(
        filtered.rgba(48, 48)[0] < 32,
        "the black square stays in place"
    );
    assert!(filtered.rgba(72, 72)[0] > 220, "white stays white");
    assert_eq!(filtered.rgba(16, 16), plain.rgba(16, 16));
    assert_eq!(filtered.rgba(110, 110), plain.rgba(110, 110));
}

#[test]
fn a_backdrop_in_a_group_reads_transparent_past_the_group() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    builder.begin_group(group(rect(32.0, 32.0, 64.0, 64.0), 1.0, &[]));
    quad(&mut builder, rect(32.0, 32.0, 64.0, 64.0), [0, 0, 0]);
    builder.backdrop(frosted(rect(32.0, 32.0, 64.0, 16.0), 4.0));
    builder.end_group();

    let pixels = present(&mut renderer, &builder.finish());

    assert!(
        pixels.rgba(64, 64)[0] < 8,
        "below the backdrop the group is black"
    );
    assert_eq!(
        pixels.rgba(16, 16),
        [255, 255, 255, 255],
        "outside it is white"
    );
    assert_eq!(renderer.groups().lent(), 0);
}

#[test]
fn a_backdrop_first_in_a_group_reads_nothing_the_last_lease_left() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    builder.begin_group(group(rect(0.0, 0.0, 64.0, 128.0), 1.0, &[]));
    quad(&mut builder, rect(0.0, 0.0, 64.0, 128.0), [255, 0, 0]);
    builder.end_group();
    builder.begin_group(group(rect(64.0, 0.0, 64.0, 128.0), 1.0, &[]));
    builder.backdrop(frosted(rect(64.0, 0.0, 64.0, 128.0), 4.0));
    quad(&mut builder, rect(100.0, 100.0, 8.0, 8.0), [0, 0, 0]);
    builder.end_group();

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(
        pixels.rgba(66, 64),
        [255, 255, 255, 255],
        "no red of the first group bleeds into the second"
    );
    assert!(
        pixels.rgba(104, 104)[0] < 8,
        "the second group's own content is drawn"
    );
}
