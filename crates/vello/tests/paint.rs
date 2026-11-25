//! Gradients as vello draws them.

mod common;

use std::sync::Arc;

use iceblink::{FrameGradient, FramePaint, FrameStop, GradientShape, StopEncoding};

use common::raster::vello;
use iceblink_testing::vector::draw::draw;
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, rect};

const SIDE: i32 = 128;

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const WHITE: [f32; 4] = [1.0; 4];

fn stops(stops: &[([f32; 4], f32)]) -> Arc<[FrameStop]> {
    stops
        .iter()
        .map(|&(color, offset)| FrameStop { color, offset })
        .collect()
}

/// Returns the square over the whole output, filled with `gradient`.
fn filled(gradient: FrameGradient) -> Scene {
    let mut scene = Scene::new(SIDE);
    let paint = scene.paint(FramePaint::Gradient(gradient));
    let path = scene.fill(rect(0.0, 0.0, 128.0, 128.0), paint);
    scene.pass(&[path]);
    scene
}

fn close(got: [u8; 4], want: [u8; 4], within: u8) -> bool {
    (0..4).all(|channel| got[channel].abs_diff(want[channel]) <= within)
}

/// Returns a conic gradient with a hard quarter per colour, red from twelve o'clock.
fn quarters(angle: f32) -> FrameGradient {
    FrameGradient {
        shape: GradientShape::Conic {
            center: [64.0, 64.0],
            angle,
        },
        encoding: StopEncoding::SRGB,
        repeat: false,
        stops: stops(&[
            (RED, 0.0),
            (RED, 0.25),
            (GREEN, 0.25),
            (GREEN, 0.5),
            (BLUE, 0.5),
            (BLUE, 0.75),
            (WHITE, 0.75),
            (WHITE, 1.0),
        ]),
    }
}

#[test]
fn a_conic_gradient_sweeps_clockwise_from_twelve_o_clock() {
    let Some(gpu) = open_device() else {
        return;
    };

    let drawn = draw(&gpu, &mut vello(&gpu), &filled(quarters(0.0)));

    assert_eq!(
        drawn.canvas.rgba(85, 43),
        [255, 0, 0, 255],
        "the first quarter"
    );
    assert_eq!(
        drawn.canvas.rgba(85, 85),
        [0, 255, 0, 255],
        "the second quarter"
    );
    assert_eq!(
        drawn.canvas.rgba(43, 85),
        [0, 0, 255, 255],
        "the third quarter"
    );
    assert_eq!(drawn.canvas.rgba(43, 43), [255; 4], "the last quarter");
}

#[test]
fn a_conic_gradient_starts_at_its_angle() {
    let Some(gpu) = open_device() else {
        return;
    };

    let drawn = draw(
        &gpu,
        &mut vello(&gpu),
        &filled(quarters(core::f32::consts::FRAC_PI_2)),
    );

    assert_eq!(
        drawn.canvas.rgba(85, 85),
        [255, 0, 0, 255],
        "red starts at three o'clock"
    );
    assert_eq!(
        drawn.canvas.rgba(85, 43),
        [255; 4],
        "and the last quarter wraps round"
    );
}

#[test]
fn an_elliptical_radial_gradient_reaches_each_radius_at_the_same_offset() {
    let Some(gpu) = open_device() else {
        return;
    };
    let gradient = FrameGradient {
        shape: GradientShape::Radial {
            center: [64.0, 64.0],
            radius: [40.0, 20.0],
        },
        encoding: StopEncoding::SRGB,
        repeat: false,
        stops: stops(&[(WHITE, 0.0), ([0.0, 0.0, 0.0, 1.0], 1.0)]),
    };

    let drawn = draw(&gpu, &mut vello(&gpu), &filled(gradient));

    let (across, down) = (drawn.canvas.rgba(84, 64), drawn.canvas.rgba(64, 74));
    assert!(
        close(across, down, 3),
        "{across:?} and {down:?} sit at one offset"
    );
    assert!(
        close(across, [128, 128, 128, 255], 4),
        "{across:?} is halfway"
    );
}

#[test]
fn an_oklab_ramp_draws_the_oklab_midpoint() {
    let Some(gpu) = open_device() else {
        return;
    };
    let oklab = |red: f32, green: f32, blue: f32| {
        let color = iceblink::color::Color::srgb(red, green, blue, 1.0)
            .to_space(iceblink::color::ColorSpace::Oklab);
        let [l, a, b] = color.components();
        [l, a, b, 1.0]
    };
    let gradient = FrameGradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [128.0, 0.0],
        },
        encoding: StopEncoding::OKLAB,
        repeat: false,
        stops: stops(&[(oklab(0.0, 0.0, 1.0), 0.0), (oklab(1.0, 1.0, 0.0), 1.0)]),
    };
    let middle = iceblink::color::interpolate(
        iceblink::color::Color::srgb(0.0, 0.0, 1.0, 1.0),
        iceblink::color::Color::srgb(1.0, 1.0, 0.0, 1.0),
        64.5 / 128.0,
        iceblink::color::Interpolation::new(iceblink::color::ColorSpace::Oklab),
    )
    .to_space(iceblink::color::ColorSpace::Srgb);
    let [red, green, blue] = middle
        .components()
        .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);

    let drawn = draw(&gpu, &mut vello(&gpu), &filled(gradient));

    let got = drawn.canvas.rgba(64, 64);
    assert!(
        close(got, [red, green, blue, 255], 3),
        "{got:?} against {:?}",
        [red, green, blue]
    );
    assert!(
        !close(got, [128, 128, 128, 255], 10),
        "the sRGB midpoint is grey"
    );
}

#[test]
fn a_ramp_to_transparent_interpolates_premultiplied() {
    let Some(gpu) = open_device() else {
        return;
    };
    let gradient = FrameGradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [128.0, 0.0],
        },
        encoding: StopEncoding::SRGB,
        repeat: false,
        stops: stops(&[(RED, 0.0), ([0.0; 4], 1.0)]),
    };

    let drawn = draw(&gpu, &mut vello(&gpu), &filled(gradient));

    // Half of opaque red over black, where a straight ramp would give a quarter.
    let got = drawn.canvas.rgba(64, 64);
    assert!(close(got, [128, 0, 0, 255], 4), "{got:?}");
}

#[test]
fn a_repeating_ramp_starts_again_past_its_end() {
    let Some(gpu) = open_device() else {
        return;
    };
    let gradient = FrameGradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [32.0, 0.0],
        },
        encoding: StopEncoding::SRGB,
        repeat: true,
        stops: stops(&[(RED, 0.0), (BLUE, 1.0)]),
    };

    let drawn = draw(&gpu, &mut vello(&gpu), &filled(gradient));

    assert!(close(
        drawn.canvas.rgba(1, 64),
        drawn.canvas.rgba(65, 64),
        2
    ));
    assert!(close(drawn.canvas.rgba(30, 64), [8, 0, 247, 255], 12));
}

#[test]
fn the_brush_moves_the_paint_inside_the_path() {
    let Some(gpu) = open_device() else {
        return;
    };
    let gradient = FrameGradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [64.0, 0.0],
        },
        encoding: StopEncoding::SRGB,
        repeat: false,
        stops: stops(&[(RED, 0.0), (BLUE, 1.0)]),
    };
    let scene = filled(gradient.clone());
    let mut moved = filled(gradient);
    moved.paths[0].brush = kurbo::Affine::translate((32.0, 0.0));

    let plain = draw(&gpu, &mut vello(&gpu), &scene);
    let brushed = draw(&gpu, &mut vello(&gpu), &moved);

    assert!(close(
        brushed.canvas.rgba(48, 64),
        plain.canvas.rgba(16, 64),
        2
    ));
}
