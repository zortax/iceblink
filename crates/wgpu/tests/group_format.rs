//! The format a group's target is held in.
//!
//! A plain group (no filter, source-over, in no other group) is held in the composed format at
//! compact precision. What it holds keeps its own targets and composites into it. Every other
//! group keeps `Rgba16Float`.

mod common;

use common::build::{FrameBuilder, area, filled, frosted, group, rect, straight};
use common::device::{SIDE, TestRenderer, draw, pair, plain_renderer, present, tolerance};
use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Point, Rect, Size};
use iceblink::{BlendMode, Filter, Frame};
use iceblink_wgpu::{GroupPrecision, Pixels};

/// The bytes of the default group budget.
const BUDGET: u64 = 256 * 1024 * 1024;

/// Adds a quad of `color` over `bounds`, with every corner rounded by `radius`.
fn quad(builder: &mut FrameBuilder, bounds: [f32; 4], color: [u8; 4], radius: f32) {
    let paint = builder.solid(straight(color[0], color[1], color[2], color[3]));
    let mut quad = filled(bounds, paint);
    quad.radii = [radius; 8];
    builder.quad(quad);
}

/// Returns a frame with a background of `background` and `content` drawn over it.
fn frame_of(background: [u8; 4], content: impl FnOnce(&mut FrameBuilder)) -> Frame {
    let mut builder = FrameBuilder::new();
    builder.frame_mut().opaque = background[3] == 255;
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), background, 0.0);
    content(&mut builder);
    builder.finish()
}

/// Draws `content` in a group over `bounds` at `opacity` through `filters`.
fn grouped(
    builder: &mut FrameBuilder,
    bounds: [f32; 4],
    opacity: f32,
    filters: &[Filter],
    content: impl FnOnce(&mut FrameBuilder),
) {
    builder.begin_group(group(bounds, opacity, filters));
    content(builder);
    builder.end_group();
}

/// Stacks `layers` translucent, rounded and off-grid quads of `alpha`.
fn stacked(builder: &mut FrameBuilder, layers: u8, alpha: u8) {
    for layer in 0..layers {
        let at = 10.3 + 7.7 * f32::from(layer);
        let color = [
            40u8.wrapping_add(layer.wrapping_mul(53)),
            200u8.wrapping_sub(layer.wrapping_mul(31)),
            90u8.wrapping_add(layer.wrapping_mul(17)),
            alpha,
        ];
        quad(builder, rect(at, at * 0.8, 61.4, 47.9), color, 9.5);
    }
}

/// Frames whose only group is plain and holds at most one translucent layer on a pixel.
fn single_layer_frames() -> Vec<(&'static str, Frame)> {
    vec![
        (
            "half opacity over two overlapping opaque quads",
            frame_of([200, 120, 60, 255], |builder| {
                grouped(builder, rect(16.0, 16.0, 96.0, 96.0), 0.5, &[], |builder| {
                    quad(
                        builder,
                        rect(16.5, 16.5, 64.0, 64.0),
                        [10, 20, 30, 255],
                        12.0,
                    );
                    quad(
                        builder,
                        rect(48.25, 48.25, 64.0, 64.0),
                        [240, 250, 5, 255],
                        12.0,
                    );
                });
            }),
        ),
        (
            "a third of opacity over one translucent quad",
            frame_of([30, 60, 220, 255], |builder| {
                grouped(
                    builder,
                    rect(0.0, 0.0, 128.0, 128.0),
                    0.33,
                    &[],
                    |builder| {
                        stacked(builder, 1, 150);
                    },
                );
            }),
        ),
        (
            "a translucent group of one quad over a translucent surface",
            frame_of([120, 30, 90, 140], |builder| {
                grouped(
                    builder,
                    rect(8.0, 8.0, 100.0, 100.0),
                    0.61,
                    &[],
                    |builder| {
                        stacked(builder, 1, 128);
                    },
                );
            }),
        ),
    ]
}

/// Returns a plain group of `layers` stacked translucent quads of `alpha`, at `opacity`.
fn stacked_frame(layers: u8, alpha: u8, opacity: f32) -> Frame {
    frame_of([250, 250, 250, 255], |builder| {
        grouped(
            builder,
            rect(0.0, 0.0, 128.0, 128.0),
            opacity,
            &[],
            |builder| {
                stacked(builder, layers, alpha);
            },
        );
    })
}

/// Frames whose groups need headroom.
fn wide_frames() -> Vec<(&'static str, Frame)> {
    let bounds = rect(8.0, 8.0, 112.0, 112.0);
    let brighter = Filter::ColorMatrix([
        1.3, 0.0, 0.0, 0.0, 0.0, 0.0, 1.3, 0.0, 0.0, 0.0, 0.0, 0.0, 1.3, 0.0, 0.0, 0.0, 0.0, 0.0,
        1.0, 0.0,
    ]);
    vec![
        (
            "a blurred group",
            frame_of([255, 255, 255, 255], |builder| {
                grouped(builder, bounds, 0.5, &[Filter::Blur(3.0)], |builder| {
                    stacked(builder, 3, 150);
                });
            }),
        ),
        (
            "a colour-matrix group",
            frame_of([255, 255, 255, 255], |builder| {
                grouped(builder, bounds, 0.5, &[brighter], |builder| {
                    stacked(builder, 3, 150);
                });
            }),
        ),
        (
            "a blend-mode group",
            frame_of([255, 255, 255, 255], |builder| {
                let mut multiply = group(bounds, 0.5, &[]);
                multiply.blend = BlendMode::MULTIPLY;
                builder.begin_group(multiply);
                stacked(builder, 3, 150);
                builder.end_group();
            }),
        ),
    ]
}

/// Plain groups that hold a group, a filtered group or a backdrop.
fn holding_frames() -> Vec<(&'static str, Frame)> {
    let bounds = rect(8.0, 8.0, 112.0, 112.0);
    vec![
        (
            "nested groups",
            frame_of([255, 255, 255, 255], |builder| {
                grouped(builder, bounds, 0.5, &[], |builder| {
                    grouped(builder, bounds, 0.5, &[], |builder| {
                        stacked(builder, 3, 150)
                    });
                });
            }),
        ),
        (
            "a group holding a blurred group",
            frame_of([255, 255, 255, 255], |builder| {
                grouped(builder, bounds, 0.7, &[], |builder| {
                    stacked(builder, 2, 150);
                    grouped(builder, bounds, 0.6, &[Filter::Blur(3.0)], |builder| {
                        stacked(builder, 3, 150);
                    });
                });
            }),
        ),
        (
            "a group holding a backdrop",
            frame_of([255, 255, 255, 255], |builder| {
                grouped(builder, bounds, 0.5, &[], |builder| {
                    stacked(builder, 3, 150);
                    builder.backdrop(frosted(rect(30.0, 30.0, 50.0, 40.0), 4.0));
                });
            }),
        ),
    ]
}

/// Returns the largest difference in any channel inside `within`, and how many pixels differ.
fn compare(first: &Pixels, second: &Pixels, within: Rect<i32, Device>) -> (u8, usize) {
    let mut largest = 0;
    let mut differing = 0;
    for y in within.origin.y..within.origin.y + within.size.height {
        for x in within.origin.x..within.origin.x + within.size.width {
            let (a, b) = (first.rgba(x, y), second.rgba(x, y));
            let delta = (0..4).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0);
            largest = largest.max(delta);
            differing += usize::from(delta > 0);
        }
    }
    (largest, differing)
}

/// Returns the whole output.
fn everywhere() -> Rect<i32, Device> {
    Rect::new(Point::new(0, 0), Size::new(SIDE as i32, SIDE as i32))
}

/// Returns two renderers, the first at compact and the second at high precision.
fn compact_and_high() -> (TestRenderer, TestRenderer) {
    let (mut compact, mut high) = pair();
    compact.set_group_precision(GroupPrecision::Compact);
    high.set_group_precision(GroupPrecision::High);
    (compact, high)
}

// Each blend into an eight-bit target rounds by up to half a step. Content with at most one
// translucent layer on a pixel differs from a sixteen-bit group by at most one step in each
// channel. Stacked translucent layers, and groups and backdrops inside the group, add their
// roundings and differ by at most two steps. On lavapipe each bound is one step wider.
#[test]
fn a_plain_group_of_one_translucent_layer_differs_by_at_most_one_step() {
    let (mut narrow, mut wide) = compact_and_high();
    for (name, frame) in single_layer_frames() {
        let (held, reference) = (present(&mut narrow, &frame), present(&mut wide, &frame));
        let (largest, differing) = compare(&held, &reference, everywhere());
        eprintln!("{name}: max channel delta {largest}, {differing} pixels differ");
        assert!(
            // Measured on lavapipe: one translucent layer differs by two steps, eight by three.
            largest <= tolerance(1),
            "{name}: a plain group in eight bits differs by {largest} steps from one in sixteen"
        );
    }
}

// At full opacity a plain group is never further from the picture with no group than a sixteen-bit
// one is.
#[test]
fn stacked_layers_in_a_plain_group_differ_by_at_most_two_steps() {
    let (mut narrow, mut wide) = compact_and_high();
    for (layers, alpha, opacity) in [(2, 150, 1.0), (3, 150, 0.33), (8, 77, 1.0), (8, 201, 0.77)] {
        let frame = stacked_frame(layers, alpha, opacity);
        let (held, reference) = (present(&mut narrow, &frame), present(&mut wide, &frame));
        let (largest, differing) = compare(&held, &reference, everywhere());
        eprintln!(
            "{layers} layers of alpha {alpha} at opacity {opacity}: max channel delta {largest}, \
             {differing} pixels differ"
        );
        assert!(
            largest <= tolerance(2),
            "{layers} layers: a plain group in eight bits differs by {largest} steps"
        );
        if opacity < 1.0 {
            continue;
        }
        let direct = frame_of([250, 250, 250, 255], |builder| {
            stacked(builder, layers, alpha);
        });
        let direct = present(&mut narrow, &direct);
        let (from_narrow, _) = compare(&held, &direct, everywhere());
        let (from_wide, _) = compare(&reference, &direct, everywhere());
        assert!(
            from_narrow <= from_wide,
            "{layers} layers: eight bits are {from_narrow} steps from no group, sixteen {from_wide}"
        );
    }
}

#[test]
fn a_group_that_needs_headroom_draws_exactly_as_at_high_precision() {
    let (mut narrow, mut wide) = compact_and_high();
    for (name, frame) in wide_frames() {
        let (held, reference) = (present(&mut narrow, &frame), present(&mut wide, &frame));
        assert_eq!(
            held.max_difference(&reference),
            0,
            "{name}: a group that needs headroom keeps its sixteen-bit target"
        );
        assert_eq!(
            narrow.groups().bytes(),
            wide.groups().bytes(),
            "{name}: and the pool holds the same bytes"
        );
    }
}

// What a plain group holds keeps its own target and composites into the plain group's target.
#[test]
fn a_plain_group_holding_groups_and_backdrops_differs_by_at_most_two_steps() {
    let (mut narrow, mut wide) = compact_and_high();
    for (name, frame) in holding_frames() {
        let (held, reference) = (present(&mut narrow, &frame), present(&mut wide, &frame));
        let (largest, differing) = compare(&held, &reference, everywhere());
        eprintln!("{name}: max channel delta {largest}, {differing} pixels differ");
        assert!(
            largest <= tolerance(2),
            "{name}: a plain group in eight bits differs by {largest} steps from one in sixteen"
        );
    }
}

// Two groups over one region: the outer one takes four bytes a texel, and the inner one, which is
// in another group, keeps eight.
#[test]
fn a_group_in_a_plain_group_keeps_its_wide_target() {
    let (mut narrow, mut wide) = compact_and_high();
    let (_, nested) = holding_frames().swap_remove(0);

    present(&mut narrow, &nested);
    present(&mut wide, &nested);

    assert_eq!(narrow.groups().bytes() * 4, wide.groups().bytes() * 3);
}

#[test]
fn a_plain_group_costs_half_the_bytes_of_a_wide_one() {
    let (mut narrow, mut wide) = compact_and_high();
    let (_, frame) = single_layer_frames().swap_remove(0);

    present(&mut narrow, &frame);
    present(&mut wide, &frame);

    assert!(narrow.groups().bytes() > 0, "the group was isolated");
    assert_eq!(
        narrow.groups().bytes() * 2,
        wide.groups().bytes(),
        "four bytes a texel against eight, over the same region"
    );
}

// A renderer holds a plain group in the composed format by default, and a new group budget keeps
// the precision it was set to.
#[test]
fn high_group_precision_holds_a_plain_group_in_sixteen_bit_float() {
    let (mut compact, mut high) = pair();
    if std::env::var("ICEBLINK_GROUP_PRECISION").is_err() {
        assert_eq!(compact.group_precision(), GroupPrecision::Compact);
    }
    compact.set_group_precision(GroupPrecision::Compact);
    high.set_group_precision(GroupPrecision::High);
    high.set_group_budget(BUDGET);
    assert_eq!(high.group_precision(), GroupPrecision::High);

    let (_, frame) = single_layer_frames().swap_remove(0);
    present(&mut compact, &frame);
    present(&mut high, &frame);

    assert!(compact.groups().bytes() > 0, "the group was isolated");
    assert_eq!(
        compact.groups().bytes() * 2,
        high.groups().bytes(),
        "Bgra8Unorm against Rgba16Float, four bytes a texel against eight"
    );
}

/// The bounds of the animated group at `phase`.
fn animated_bounds(phase: f32) -> [f32; 4] {
    rect(40.0 - 20.0 * phase, 30.0, 41.0, 18.0)
}

/// Returns the frame of the animated group at `phase`, drawn over `damage`.
fn animated(phase: f32, damage: Option<DamageSet>) -> Frame {
    let mut builder = match damage {
        Some(damage) => FrameBuilder::new().damage(damage),
        None => FrameBuilder::new(),
    };
    quad(&mut builder, rect(0.0, 0.0, 128.0, 128.0), [255; 4], 0.0);
    let at = animated_bounds(phase);
    grouped(&mut builder, at, 0.4 + 0.6 * phase, &[], |builder| {
        quad(builder, at, [40, 120, 200, 255], 4.0);
        quad(
            builder,
            rect(at[0] + 8.0, at[1] + 6.0, 12.0, 6.0),
            [255, 255, 255, 160],
            0.0,
        );
    });
    builder.finish()
}

// The format does not change from frame to frame.
#[test]
fn an_animated_plain_group_keeps_one_texture_under_partial_damage() {
    let mut renderer = plain_renderer();
    renderer.set_group_precision(GroupPrecision::Compact);
    present(&mut renderer, &animated(0.0, None));
    let (allocations, bytes) = (renderer.groups().allocations(), renderer.groups().bytes());
    for step in 1..=20 {
        let (from, to) = (0.9 * (step - 1) as f32 / 20.0, 0.9 * step as f32 / 20.0);
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
        let outcome = draw(&mut renderer, &animated(to, Some(damage)));
        assert!(outcome.stats().is_some());
        assert_eq!(
            renderer.groups().allocations(),
            allocations,
            "step {step} allocated a texture"
        );
        assert_eq!(renderer.groups().bytes(), bytes, "step {step}");
    }
}

/// Returns a translucent panel with stacked layers at its top left and, when `corner` is set, a
/// nested group and a backdrop at its right edge.
fn panel(corner: bool, damage: Option<DamageSet>) -> Frame {
    let mut builder = match damage {
        Some(damage) => FrameBuilder::new().damage(damage),
        None => FrameBuilder::new(),
    };
    quad(
        &mut builder,
        rect(0.0, 0.0, 128.0, 128.0),
        [250, 250, 250, 255],
        0.0,
    );
    grouped(
        &mut builder,
        rect(0.0, 0.0, 128.0, 128.0),
        0.6,
        &[],
        |builder| {
            stacked(builder, 4, 150);
            if corner {
                let bounds = rect(96.0, 96.0, 28.0, 28.0);
                grouped(builder, bounds, 0.5, &[], |builder| {
                    quad(builder, bounds, [20, 40, 200, 255], 3.0);
                });
                builder.backdrop(frosted(rect(100.0, 40.0, 24.0, 20.0), 2.0));
            }
        },
    );
    builder.finish()
}

// A frame whose damage misses what a group holds leaves it out, as a compositor does. The group
// keeps its format, so the frame reuses the group's texture and draws the pixels a whole frame
// does.
#[test]
fn a_group_keeps_its_format_when_the_damage_leaves_out_what_it_holds() {
    let (mut whole, mut partial) = pair();
    whole.set_group_precision(GroupPrecision::Compact);
    partial.set_group_precision(GroupPrecision::Compact);
    let reference = present(&mut whole, &panel(true, None));
    present(&mut partial, &panel(true, None));
    let (allocations, bytes) = (partial.groups().allocations(), partial.groups().bytes());

    let within = Rect::new(Point::new(10, 8), Size::new(60, 50));
    let pixels = present(&mut partial, &panel(false, Some(area(10, 8, 60, 50))));

    assert_eq!(
        partial.groups().allocations(),
        allocations,
        "the partial frame allocated a texture of another format"
    );
    assert_eq!(partial.groups().bytes(), bytes);
    assert_eq!(
        compare(&reference, &pixels, within).0,
        0,
        "a partial redraw differs from a whole one inside {within:?}"
    );
}
