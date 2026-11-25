//! Gradient paints: linear, radial and conic ramps, and stops in Oklab.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, rect};
use common::device::{plain_renderer, present};
use iceblink::{FrameGradient, FramePaint, FrameStop, GradientShape, StopEncoding};

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Returns a gradient paint of `shape` through `stops`, each a straight colour and an offset.
fn ramp(shape: GradientShape, encoding: StopEncoding, stops: &[([f32; 4], f32)]) -> FramePaint {
    FramePaint::Gradient(FrameGradient {
        shape,
        encoding,
        repeat: false,
        stops: Arc::from(
            stops
                .iter()
                .map(|(color, offset)| FrameStop {
                    color: *color,
                    offset: *offset,
                })
                .collect::<Vec<_>>(),
        ),
    })
}

#[test]
fn a_linear_ramp_runs_from_its_start_to_its_end() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let paint = builder.paint(ramp(
        GradientShape::Linear {
            start: [0.0, 0.0],
            end: [128.0, 0.0],
        },
        StopEncoding::SRGB,
        &[(RED, 0.0), (BLUE, 1.0)],
    ));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 16.0), paint));

    let pixels = present(&mut renderer, &builder.finish());

    let [r0, _, b0, _] = pixels.rgba(1, 8);
    let [r1, _, b1, _] = pixels.rgba(64, 8);
    let [r2, _, b2, _] = pixels.rgba(126, 8);
    assert!(r0 > 240 && b0 < 15);
    assert!(
        (110..=145).contains(&r1) && (110..=145).contains(&b1),
        "{r1} {b1}"
    );
    assert!(r2 < 15 && b2 > 240);
}

#[test]
fn a_radial_ramp_and_a_conic_ramp_follow_their_geometry() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let radial = builder.paint(ramp(
        GradientShape::Radial {
            center: [32.0, 32.0],
            radius: [32.0, 32.0],
        },
        StopEncoding::SRGB,
        &[(RED, 0.0), (BLUE, 1.0)],
    ));
    let conic = builder.paint(ramp(
        GradientShape::Conic {
            center: [32.0, 32.0],
            angle: 0.0,
        },
        StopEncoding::SRGB,
        &[(RED, 0.0), (BLUE, 1.0)],
    ));
    builder.quad(filled(rect(0.0, 0.0, 64.0, 64.0), radial));
    let mut right = filled(rect(64.0, 0.0, 64.0, 64.0), conic);
    right.paint_origin = [64.0, 0.0];
    builder.quad(right);

    let pixels = present(&mut renderer, &builder.finish());

    assert!(pixels.rgba(32, 32)[0] > 240, "red at the centre");
    assert!(pixels.rgba(32, 1)[2] > 200, "blue at the radius");
    // The conic sweep starts at twelve o'clock and runs clockwise.
    let right_of_centre = pixels.rgba(96 + 20, 32);
    let left_of_centre = pixels.rgba(96 - 20, 32);
    assert!(
        right_of_centre[0] > left_of_centre[0],
        "a quarter turn is redder than three"
    );
}

#[test]
fn oklab_stops_are_decoded_to_encoded_srgb() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    // White in Oklab is lightness one, with no chroma.
    let paint = builder.paint(ramp(
        GradientShape::Linear {
            start: [0.0, 0.0],
            end: [128.0, 0.0],
        },
        StopEncoding::OKLAB,
        &[([1.0, 0.0, 0.0, 1.0], 0.0), ([1.0, 0.0, 0.0, 1.0], 1.0)],
    ));
    builder.quad(filled(rect(0.0, 0.0, 128.0, 16.0), paint));

    let pixels = present(&mut renderer, &builder.finish());

    let [r, g, b, a] = pixels.rgba(64, 8);
    assert!(r > 250 && g > 250 && b > 250 && a == 255, "{r} {g} {b} {a}");
}
