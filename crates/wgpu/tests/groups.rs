//! Groups: content drawn into a target of its own and composited once.

mod common;

use common::build::{FrameBuilder, area, filled, group, opaque, quad, rect};
use common::device::{TestRenderer, pair, plain_renderer, present};
use iceblink::damage::DamageSet;
use iceblink::{ClipEntry, ClipLinkEntry, Filter, Frame, FrameClipShape, ROUND};
use iceblink_wgpu::GroupPrecision;

#[test]
fn a_translucent_group_composites_its_overlap_once() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let red = builder.solid(opaque(255, 0, 0));
    builder.begin_group(group(rect(0.0, 0.0, 96.0, 64.0), 0.5, &[]));
    builder.quad(filled(rect(0.0, 0.0, 64.0, 64.0), red));
    builder.quad(filled(rect(32.0, 0.0, 64.0, 64.0), red));
    builder.end_group();

    let pixels = present(&mut renderer, &builder.finish());

    let alone = pixels.rgba(16, 32);
    let overlap = pixels.rgba(48, 32);
    assert!((126..=129).contains(&alone[3]), "{alone:?}");
    assert_eq!(alone, overlap, "the overlap is not drawn twice");
}

#[test]
fn nested_groups_multiply_their_opacities_and_a_group_clip_cuts_the_composite() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let green = builder.solid(opaque(0, 255, 0));
    let clip = builder.clip(ClipEntry {
        links: [ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::RoundedRect {
                rect: rect(0.0, 0.0, 32.0, 64.0),
                radii: [0.0; 8],
                shapes: ROUND,
            },
        }]
        .into_iter()
        .collect(),
    });
    let mut outer = group(rect(0.0, 0.0, 64.0, 64.0), 0.5, &[]);
    outer.clip = clip.0;
    builder.begin_group(outer);
    builder.begin_group(group(rect(0.0, 0.0, 64.0, 64.0), 0.5, &[]));
    builder.quad(filled(rect(0.0, 0.0, 64.0, 64.0), green));
    builder.end_group();
    builder.end_group();

    let pixels = present(&mut renderer, &builder.finish());

    let inside = pixels.rgba(16, 32)[3];
    assert!((62..=66).contains(&inside), "a quarter: {inside}");
    assert_eq!(pixels.rgba(48, 32)[3], 0, "the clip cuts the composite");
}

// The empty group follows one that leaves red in the target the pool lends again.
#[test]
fn an_empty_group_draws_nothing() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    builder.begin_group(group(rect(64.0, 64.0, 64.0, 64.0), 1.0, &[]));
    quad(&mut builder, rect(64.0, 64.0, 64.0, 64.0), [255, 0, 0]);
    builder.end_group();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    builder.begin_group(group(rect(64.0, 64.0, 64.0, 64.0), 1.0, &[]));
    builder.end_group();

    let pixels = present(&mut renderer, &builder.finish());

    assert_eq!(pixels.rgba(96, 96), [255, 255, 255, 255]);
}

/// Draws a black quad over `bounds` in a group of its own at `opacity` through `filters`, over a
/// white output.
fn black_group(bounds: [f32; 4], opacity: f32, filters: &[Filter]) -> Frame {
    let mut builder = FrameBuilder::new();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    builder.begin_group(group(bounds, opacity, filters));
    quad(&mut builder, bounds, [0, 0, 0]);
    builder.end_group();
    builder.finish()
}

fn channels(pixel: [u8; 4]) -> [u8; 3] {
    [pixel[0], pixel[1], pixel[2]]
}

#[test]
fn a_group_nested_four_deep_composites_every_level() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    for _ in 0..4 {
        builder.begin_group(group(rect(8.0, 8.0, 112.0, 112.0), 0.5, &[]));
    }
    quad(&mut builder, rect(16.0, 16.0, 96.0, 96.0), [0, 0, 0]);
    for _ in 0..4 {
        builder.end_group();
    }

    let pixels = present(&mut renderer, &builder.finish());

    let inside = pixels.rgba(64, 64)[0];
    assert!(
        inside.abs_diff(239) <= 2,
        "four halvings leave 15/16 of white: {inside}"
    );
    assert!(
        renderer.groups().peak() >= 4,
        "four levels were open at once"
    );
}

#[test]
fn a_colour_matrix_costs_no_target_beyond_the_group_itself() {
    let grey = Filter::ColorMatrix([
        0.2126, 0.7152, 0.0722, 0.0, 0.0, 0.2126, 0.7152, 0.0722, 0.0, 0.0, 0.2126, 0.7152, 0.0722,
        0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
    ]);
    let invert = Filter::ColorMatrix([
        -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
    ]);
    let mut renderer = plain_renderer();

    let pixels = present(
        &mut renderer,
        &black_group(rect(16.0, 16.0, 96.0, 96.0), 1.0, &[grey, invert]),
    );

    assert_eq!(
        channels(pixels.rgba(64, 64)),
        [255; 3],
        "grey then inverted is white"
    );
    assert_eq!(
        renderer.groups().peak(),
        1,
        "the matrices travel with the composite"
    );
}

#[test]
fn the_pool_lends_a_finished_group_s_target_to_the_next() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    for row in 0..8 {
        let bounds = rect(0.0, row as f32 * 16.0, 128.0, 16.0);
        builder.begin_group(group(bounds, 0.5, &[]));
        quad(&mut builder, bounds, [0, 0, 0]);
        builder.end_group();
    }

    present(&mut renderer, &builder.finish());

    assert_eq!(
        renderer.groups().peak(),
        1,
        "eight groups in sequence held one target"
    );
}

#[test]
fn a_group_the_pool_cannot_lend_a_target_for_draws_flat() {
    let mut renderer = plain_renderer();
    renderer.set_group_budget(0);

    let pixels = present(
        &mut renderer,
        &black_group(rect(32.0, 32.0, 64.0, 64.0), 1.0, &[]),
    );

    assert_eq!(renderer.groups().lent(), 0, "nothing was lent");
    assert_eq!(
        channels(pixels.rgba(64, 64)),
        [0; 3],
        "the content is drawn"
    );
    assert_eq!(pixels.rgba(16, 16), [255, 255, 255, 255]);
}

/// Returns the bytes of the one target a group over `bounds` takes at high precision.
fn one_target(bounds: [f32; 4]) -> u64 {
    let mut renderer = plain_renderer();
    renderer.set_group_precision(GroupPrecision::High);
    present(&mut renderer, &black_group(bounds, 1.0, &[]));
    renderer.groups().bytes()
}

/// Returns a renderer whose budget holds one half-resolution target of a group over `bounds`.
fn halved(bounds: [f32; 4]) -> TestRenderer {
    let quarter = one_target(bounds) / 4;
    let mut renderer = plain_renderer();
    renderer.set_group_precision(GroupPrecision::High);
    renderer.set_group_budget(quarter);
    renderer
}

#[test]
fn a_half_resolution_target_is_composited_in_its_place() {
    let bounds = rect(32.0, 32.0, 64.0, 64.0);
    let mut renderer = halved(bounds);

    let reduced = present(&mut renderer, &black_group(bounds, 1.0, &[]));

    assert_eq!(
        renderer.groups().degraded(),
        1,
        "the budget forced one reduction"
    );
    assert_eq!(
        channels(reduced.rgba(64, 64)),
        [0; 3],
        "the group's own colour"
    );
    assert_eq!(
        reduced.rgba(16, 16),
        [255, 255, 255, 255],
        "nothing outside it"
    );
}

#[test]
fn a_half_resolution_group_ending_on_an_odd_pixel_keeps_its_last_row_and_column() {
    let bounds = rect(32.0, 32.0, 63.0, 63.0);
    let mut renderer = halved(bounds);

    let reduced = present(&mut renderer, &black_group(bounds, 1.0, &[]));

    assert_eq!(
        renderer.groups().degraded(),
        1,
        "the group is at half resolution"
    );
    let (column, row) = (reduced.rgba(94, 64)[0], reduced.rgba(64, 94)[0]);
    assert!(
        column < 128 && row < 128,
        "the last column {column} and row {row} are dark"
    );
}

fn animated_bounds(phase: f32) -> [f32; 4] {
    rect(40.0 - 20.0 * phase, 30.0, 41.0, 18.0)
}

/// Returns the animated group at `phase` over a white output, drawn over `damage`.
fn animated(phase: f32, damage: DamageSet) -> Frame {
    let mut builder = FrameBuilder::new().damage(damage);
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 3]);
    let at = animated_bounds(phase);
    builder.begin_group(group(at, 0.4 + 0.6 * phase, &[]));
    quad(&mut builder, at, [40, 120, 200]);
    quad(
        &mut builder,
        rect(at[0] + 8.0, at[1] + 6.0, 12.0, 6.0),
        [255; 3],
    );
    builder.end_group();
    builder.finish()
}

/// Returns the damage of the animated group moving from `from` to `to`.
fn moved(from: f32, to: f32) -> DamageSet {
    let mut damage = DamageSet::new();
    for phase in [from, to] {
        let [x, y, w, h] = animated_bounds(phase);
        damage.absorb_set(&area(
            x.floor() as i32,
            y.floor() as i32,
            w.ceil() as i32 + 1,
            h.ceil() as i32 + 1,
        ));
    }
    damage
}

// The renderer that drew every step, over the whole output or over each step's damage, ends on the
// picture a fresh renderer draws of the last step.
#[test]
fn a_group_animated_over_twenty_frames_ends_as_a_fresh_renderer_draws_it() {
    for partial in [false, true] {
        let (mut animating, mut fresh) = pair();
        let mut last = present(&mut animating, &animated(0.0, DamageSet::full()));
        for step in 1..=20 {
            let (from, to) = (0.9 * (step - 1) as f32 / 20.0, 0.9 * step as f32 / 20.0);
            let damage = if partial {
                moved(from, to)
            } else {
                DamageSet::full()
            };
            last = present(&mut animating, &animated(to, damage));
        }

        let built = present(&mut fresh, &animated(0.9, DamageSet::full()));

        assert_eq!(
            last.max_difference(&built),
            0,
            "partial damage {partial}: the last animated frame differs from a fresh one"
        );
        assert_eq!(fresh.groups().degraded(), 0);
    }
}
