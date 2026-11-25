//! Pool targets cover the region a group or a filter needs, and no more.

mod common;

use std::sync::MutexGuard;

use common::build::{FrameBuilder, frosted, group, quad, rect};
use common::device::device_lock;
use common::present::{Rig, target};
use iceblink::Renderer as _;
use iceblink::damage::DamageSet;
use iceblink::geom::{Device, Point, Rect};
use iceblink::{Color, Filter, Frame};
use iceblink_wgpu::{Builder, Pixels, wgpu};

/// The side of the output, in pixels.
const EXTENT: u32 = 512;

/// Returns a rig over an offscreen renderer of the side [`EXTENT`], holding `guard` if given.
fn rig(guard: Option<MutexGuard<'static, ()>>) -> Rig {
    let renderer = Builder::new()
        .offscreen([EXTENT, EXTENT], wgpu::TextureFormat::Bgra8Unorm)
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"));
    let mut rig = match guard {
        Some(guard) => Rig::new(renderer, guard),
        None => Rig::beside(renderer),
    };
    rig.configure(target(EXTENT));
    rig
}

/// Returns a rig that holds the device lock and a second rig beside it.
fn pair() -> (Rig, Rig) {
    let first = rig(Some(device_lock()));
    (first, rig(None))
}

/// Returns the frame of `builder`, with every batch's ink the whole output.
// The builder cuts inks to the side of the other tests' outputs.
fn finish(builder: FrameBuilder) -> Frame {
    let mut frame = builder.finish();
    for batch in &mut frame.batches {
        batch.ink = area(0, 0, EXTENT as i32, EXTENT as i32);
    }
    frame
}

/// Returns a builder of a frame of the side [`EXTENT`].
fn builder() -> FrameBuilder {
    FrameBuilder::sized(EXTENT, EXTENT)
}

/// Returns the whole output.
fn whole() -> [f32; 4] {
    rect(0.0, 0.0, EXTENT as f32, EXTENT as f32)
}

/// Returns the device rectangle `[x, y, width, height]`.
fn area(x: i32, y: i32, width: i32, height: i32) -> Rect<i32, Device> {
    common::build::area(x, y, width, height).rects()[0]
}

/// Returns the largest difference in any channel between `first` and `second` inside `within`.
fn difference_in(first: &Pixels, second: &Pixels, within: Rect<i32, Device>) -> u8 {
    let mut largest = 0;
    for y in within.origin.y..within.origin.y + within.size.height {
        for x in within.origin.x..within.origin.x + within.size.width {
            let (a, b) = (first.rgba(x, y), second.rgba(x, y));
            for channel in 0..4 {
                largest = largest.max(a[channel].abs_diff(b[channel]));
            }
        }
    }
    largest
}

/// Returns `frame` with the damage `rects`, closed over the read extents of its reading
/// composites as a compositor closes it.
fn with_damage(mut frame: Frame, rects: &[Rect<i32, Device>]) -> Frame {
    let mut damage = DamageSet::new();
    for rect in rects {
        damage.absorb(*rect);
    }
    let extents: Vec<Rect<i32, Device>> = frame
        .groups
        .iter()
        .filter(|group| !group.filters.is_empty())
        .map(|group| group.bounds)
        .chain(frame.backdrops.iter().map(|backdrop| backdrop.source))
        .filter_map(|[x, y, width, height]| {
            Rect::from_corners(
                Point::new(x.floor() as i32, y.floor() as i32),
                Point::new((x + width).ceil() as i32, (y + height).ceil() as i32),
            )
            .intersection(area(0, 0, EXTENT as i32, EXTENT as i32))
        })
        .collect();
    loop {
        let reached = extents.iter().find(|extent| {
            damage.intersects(**extent)
                && !damage
                    .rects()
                    .iter()
                    .any(|rect| rect.contains_rect(**extent))
        });
        match reached {
            Some(extent) => damage.absorb_whole(*extent),
            None => break,
        }
    }
    frame.damage.draw = damage;
    frame.damage.present = damage;
    frame
}

/// Returns a frame of one dark blue quad over the whole output.
fn other() -> Frame {
    let mut builder = builder();
    quad(&mut builder, whole(), [10, 10, 90]);
    finish(builder)
}

/// Returns a frame of blurred, shadowed and backdrop-holding groups over white.
fn filtered() -> Frame {
    let mut builder = builder();
    quad(&mut builder, whole(), [255; 3]);
    builder.begin_group(group(
        rect(20.0, 20.0, 40.0, 40.0),
        1.0,
        &[Filter::Blur(6.0)],
    ));
    quad(&mut builder, rect(20.0, 20.0, 40.0, 40.0), [200, 20, 20]);
    builder.end_group();
    let shadow = Filter::DropShadow {
        offset: [12.0, 6.0],
        std_dev: 3.0,
        color: Color([0.0, 0.0, 0.0, 0.9]),
    };
    builder.begin_group(group(rect(20.0, 100.0, 30.0, 30.0), 1.0, &[shadow]));
    quad(&mut builder, rect(20.0, 100.0, 30.0, 30.0), [20, 160, 40]);
    builder.end_group();
    builder.begin_group(group(
        rect(100.0, 200.0, 160.0, 100.0),
        1.0,
        &[Filter::Blur(4.0)],
    ));
    quad(
        &mut builder,
        rect(100.0, 200.0, 80.0, 100.0),
        [250, 250, 250],
    );
    quad(&mut builder, rect(180.0, 200.0, 80.0, 100.0), [0, 0, 0]);
    builder.backdrop(frosted(rect(150.0, 220.0, 60.0, 40.0), 12.0));
    builder.end_group();
    finish(builder)
}

#[test]
fn a_partial_redraw_of_filtered_groups_draws_the_pixels_a_whole_one_does() {
    let (mut whole, mut partial) = pair();
    let reference = whole.present(&filtered());
    partial.present(&other());
    let rects = [
        area(64, 20, 16, 40),
        area(52, 100, 20, 40),
        area(130, 230, 18, 10),
        area(400, 400, 16, 16),
    ];
    let leases = partial.groups().leases();

    let pixels = partial.present(&with_damage(filtered(), &rects));

    assert!(
        partial.groups().leases() > leases,
        "the partial frame isolated something"
    );
    for rect in rects {
        assert_eq!(
            difference_in(&reference, &pixels, rect),
            0,
            "a partial redraw differs from a whole one inside {rect:?}"
        );
    }
}

/// Returns a half-transparent group of a blue quad at `x` and `width` over white.
fn small_group(x: f32, width: f32) -> Frame {
    let mut builder = builder();
    quad(&mut builder, whole(), [255; 3]);
    builder.begin_group(group(rect(x, 40.0, width, 40.0), 0.5, &[]));
    quad(&mut builder, rect(x, 40.0, width, 40.0), [0, 0, 200]);
    builder.end_group();
    finish(builder)
}

/// Returns a half-transparent group of a blue quad over the whole output.
fn whole_group() -> Frame {
    let mut builder = builder();
    builder.begin_group(group(whole(), 0.5, &[]));
    quad(&mut builder, whole(), [0, 0, 200]);
    builder.end_group();
    finish(builder)
}

#[test]
fn a_group_takes_a_target_of_its_size_class_cut_to_the_damage() {
    let guard = device_lock();
    let mut rig = rig(Some(guard));
    rig.present(&small_group(40.0, 40.0));
    let small = rig.groups().bytes();
    assert!(rig.groups().leases() > 0 && small > 0);

    rig.set_group_budget(256 << 20);
    rig.present(&whole_group());
    let large = rig.groups().bytes();
    assert!(
        small * 4 <= large,
        "a small group takes {small} bytes and the output {large}"
    );

    rig.set_group_budget(256 << 20);
    rig.present(&with_damage(whole_group(), &[area(200, 200, 16, 16)]));
    assert_eq!(
        rig.groups().bytes(),
        small,
        "the target covers the damaged rectangle and its guard"
    );
}

#[test]
fn a_group_that_moves_and_grows_inside_its_size_class_allocates_nothing() {
    let guard = device_lock();
    let mut rig = rig(Some(guard));
    rig.present(&small_group(40.0, 40.0));
    let allocations = rig.groups().allocations();
    assert!(allocations > 0);

    for frame in 1..=30 {
        let step = frame as f32;
        rig.present(&small_group(40.0 + 3.0 * step, 40.0 + 2.0 * step));
    }

    assert_eq!(rig.groups().allocations(), allocations);
}

#[test]
fn a_target_no_lease_uses_is_kept_for_idle_frames_and_then_freed() {
    let guard = device_lock();
    let mut rig = rig(Some(guard));
    rig.present(&small_group(40.0, 40.0));
    assert!(rig.groups().bytes() > 0);
    let plain = || {
        let mut builder = builder();
        quad(&mut builder, whole(), [255; 3]);
        finish(builder)
    };

    let mut idle = 0;
    while rig.groups().bytes() > 0 {
        rig.present(&plain());
        idle += 1;
        assert!(idle <= 1000, "the unused target was never freed");
    }

    assert!(idle > 1, "the target was freed by the first idle frame");
}

/// Returns quads, a blurred group and a backdrop against the right and bottom edges.
fn at_the_far_edges() -> Frame {
    let mut builder = builder();
    quad(&mut builder, whole(), [255; 3]);
    quad(
        &mut builder,
        rect(480.0, 0.0, 32.0, EXTENT as f32),
        [20, 40, 220],
    );
    quad(
        &mut builder,
        rect(0.0, 480.0, EXTENT as f32, 32.0),
        [220, 40, 20],
    );
    let edge = rect(0.0, 400.0, EXTENT as f32, 112.0);
    builder.begin_group(group(edge, 1.0, &[Filter::Blur(6.0)]));
    quad(&mut builder, edge, [30, 160, 60]);
    builder.end_group();
    builder.backdrop(frosted(rect(440.0, 200.0, 72.0, 312.0), 8.0));
    finish(builder)
}

#[test]
fn a_partial_redraw_at_the_far_edges_draws_the_pixels_a_whole_one_does() {
    let (mut whole, mut partial) = pair();
    let reference = whole.present(&at_the_far_edges());
    let rects = [area(480, 440, 32, 40), area(200, 480, 40, 32)];
    partial.present(&other());
    for _ in 0..2 {
        let pixels = partial.present(&with_damage(at_the_far_edges(), &rects));
        for rect in rects {
            assert_eq!(
                difference_in(&reference, &pixels, rect),
                0,
                "a partial redraw differs from a whole one inside {rect:?}"
            );
        }
        partial.present(&at_the_far_edges());
    }
}
