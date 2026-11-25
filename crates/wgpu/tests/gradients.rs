//! Gradient stops in Oklab.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, rect};
use common::device::{plain_renderer, present};
use iceblink::{FrameGradient, FramePaint, FrameStop, GradientShape, StopEncoding};

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
