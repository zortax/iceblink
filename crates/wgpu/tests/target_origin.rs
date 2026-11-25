//! Pool targets that start away from the output's origin draw the same pixels.

mod common;

use common::build::{FrameBuilder, frosted, group, quad, rect};
use common::device::{SIDE, device_lock, offscreen};
use common::present::Rig;
use iceblink::geom::Size;
use iceblink::{Color, Filter, Frame};
use iceblink_wgpu::wgpu;

/// How far the moved renderer starts every target above and left of the output, in pixels.
const MARGIN: i32 = 6;

/// A colour matrix that inverts the colour channels.
const INVERT: [f32; 20] = [
    -1.0, 0.0, 0.0, 0.0, 1.0, //
    0.0, -1.0, 0.0, 0.0, 1.0, //
    0.0, 0.0, -1.0, 0.0, 1.0, //
    0.0, 0.0, 0.0, 1.0, 0.0,
];

/// Returns a frame of filtered groups, a backdrop and a group holding a backdrop.
fn frame() -> Frame {
    let mut builder = FrameBuilder::new();
    quad(
        &mut builder,
        rect(0.0, 0.0, SIDE as f32, SIDE as f32),
        [255; 3],
    );
    quad(
        &mut builder,
        rect(0.0, 0.0, 64.0, SIDE as f32),
        [20, 40, 200],
    );

    builder.begin_group(group(
        rect(4.0, 4.0, 40.0, 40.0),
        0.7,
        &[Filter::ColorMatrix(INVERT), Filter::Blur(3.0)],
    ));
    quad(&mut builder, rect(8.0, 8.0, 30.0, 30.0), [250, 200, 10]);
    builder.end_group();

    let shadow = Filter::DropShadow {
        offset: [6.0, 5.0],
        std_dev: 3.0,
        color: Color([0.0, 0.0, 0.0, 0.8]),
    };
    builder.begin_group(group(rect(72.0, 8.0, 32.0, 32.0), 1.0, &[shadow]));
    quad(&mut builder, rect(72.0, 8.0, 32.0, 32.0), [200, 30, 30]);
    builder.end_group();

    builder.backdrop(frosted(rect(40.0, 52.0, 48.0, 24.0), 5.0));

    builder.begin_group(group(rect(8.0, 84.0, 112.0, 40.0), 0.9, &[]));
    quad(&mut builder, rect(8.0, 84.0, 56.0, 40.0), [10, 160, 60]);
    builder.backdrop(frosted(rect(48.0, 92.0, 40.0, 24.0), 4.0));
    quad(&mut builder, rect(100.0, 100.0, 8.0, 8.0), [0, 0, 0]);
    builder.end_group();
    builder.finish()
}

#[test]
fn a_target_away_from_the_output_origin_draws_the_same_pixels() {
    let mut reference = Rig::new(offscreen(wgpu::TextureFormat::Bgra8Unorm), device_lock());
    let mut moved = Rig::beside(offscreen(wgpu::TextureFormat::Bgra8Unorm));
    reference.set_target_margin(0);
    moved.set_target_margin(MARGIN);

    let covered = reference.groups().covered();
    assert_eq!(
        moved.groups().covered(),
        Size::new(covered.width + MARGIN, covered.height + MARGIN),
        "every target starts {MARGIN} pixels above and left of the output"
    );

    let expected = reference.present(&frame());
    let pixels = moved.present(&frame());

    assert!(moved.groups().leases() > 0, "the frame isolated something");
    // lavapipe rounds the filtered samples of the blurred group by one step at another origin.
    assert!(
        expected.max_difference(&pixels) <= 1,
        "moving the targets' origin moved a pixel by {}",
        expected.max_difference(&pixels)
    );
}
