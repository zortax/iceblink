//! Frame paints as straight colours.

use std::sync::Arc;

use iceblink::{Color, FrameGradient, FramePaint, FrameStop, GradientShape, StopEncoding};
use iceblink_wgpu::vector::paint::{flat, srgb_stops, straight};

fn gradient(encoding: StopEncoding, stops: &[([f32; 4], f32)]) -> FrameGradient {
    FrameGradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
        },
        encoding,
        repeat: false,
        stops: stops
            .iter()
            .map(|&(color, offset)| FrameStop { color, offset })
            .collect::<Arc<[_]>>(),
    }
}

#[test]
fn a_premultiplied_colour_comes_back_straight() {
    assert_eq!(straight(Color([0.25, 0.0, 0.5, 0.5])), [0.5, 0.0, 1.0, 0.5]);
    assert_eq!(straight(Color::TRANSPARENT), [0.0; 4]);
}

#[test]
fn srgb_stops_pass_through_unpremultiplied() {
    let ramp = gradient(
        StopEncoding::SRGB,
        &[([0.0, 0.0, 0.5, 0.5], 0.0), ([1.0, 1.0, 0.0, 1.0], 1.0)],
    );
    let mut stops = Vec::new();

    srgb_stops(&ramp, &mut stops);

    assert_eq!(
        stops,
        [(0.0, [0.0, 0.0, 1.0, 0.5]), (1.0, [1.0, 1.0, 0.0, 1.0])]
    );
}

#[test]
fn oklab_stops_are_densified_into_srgb() {
    // Blue and yellow, as Oklab.
    let blue = iceblink::color::Color::srgb(0.0, 0.0, 1.0, 1.0)
        .to_space(iceblink::color::ColorSpace::Oklab);
    let yellow = iceblink::color::Color::srgb(1.0, 1.0, 0.0, 1.0)
        .to_space(iceblink::color::ColorSpace::Oklab);
    let stop = |color: iceblink::color::Color| {
        let [l, a, b] = color.components();
        [l, a, b, 1.0]
    };
    let ramp = gradient(
        StopEncoding::OKLAB,
        &[(stop(blue), 0.0), (stop(yellow), 1.0)],
    );
    let mut stops = Vec::new();

    srgb_stops(&ramp, &mut stops);

    assert!(stops.len() > 2, "an Oklab ramp is a curve in sRGB");
    let [red, green, blue, alpha] = stops[0].1;
    assert!(red < 0.01 && green < 0.01 && blue > 0.99 && alpha == 1.0);
}

#[test]
fn a_flat_paint_is_the_mean_colour_of_its_ramp() {
    let ramp = gradient(
        StopEncoding::SRGB,
        &[([0.0, 0.0, 0.0, 1.0], 0.0), ([1.0, 1.0, 1.0, 1.0], 1.0)],
    );

    assert_eq!(
        flat(&FramePaint::Gradient(ramp)),
        Some([0.5, 0.5, 0.5, 1.0])
    );
    assert_eq!(flat(&FramePaint::None), None);
    assert_eq!(
        flat(&FramePaint::Sides([
            Color([1.0, 0.0, 0.0, 1.0]),
            Color::TRANSPARENT,
            Color::TRANSPARENT,
            Color::TRANSPARENT,
        ])),
        Some([1.0, 0.0, 0.0, 1.0])
    );
}
