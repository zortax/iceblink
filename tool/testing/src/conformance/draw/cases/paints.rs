//! Gradients in each stop encoding, shape and extension.

use std::sync::Arc;

use iceblink::{FrameGradient, FramePaint, FrameStop, GradientShape, StopEncoding};

use crate::conformance::draw::MakeTarget;
use crate::conformance::draw::builder::{FrameBuilder, filled, opaque};
use crate::conformance::draw::canvas::{Canvas, SIZE, golden};
use crate::conformance::draw::cases::cell;
use crate::golden::Tolerance;

/// Interpolated colours: a pixel may differ by two steps anywhere along a ramp.
const RAMPS: Tolerance = Tolerance {
    max_channel: 2,
    max_pixels: 0,
};

fn stop(color: [f32; 4], offset: f32) -> FrameStop {
    FrameStop { color, offset }
}

fn gradient(
    shape: GradientShape,
    encoding: StopEncoding,
    repeat: bool,
    stops: Vec<FrameStop>,
) -> FramePaint {
    FramePaint::Gradient(FrameGradient {
        shape,
        encoding,
        repeat,
        stops: Arc::from(stops),
    })
}

/// Linear ramps in the three stop encodings, radial and conic ramps, a repeating ramp, a hard
/// stop and translucent stops, each in its own cell.
pub fn gradients(make: MakeTarget<'_>) {
    let Some(mut canvas) = Canvas::open(make) else {
        return;
    };
    let mut builder = FrameBuilder::new(SIZE);
    let white = builder.solid(opaque(255, 255, 255));
    builder.quad(filled([0.0, 0.0, 128.0, 128.0], white));

    let across = |index: usize| {
        let [x, y, width, _] = cell(index);
        GradientShape::Linear {
            start: [x, y],
            end: [x + width, y],
        }
    };
    let centre = |index: usize| {
        let [x, y, width, height] = cell(index);
        [x + width / 2.0, y + height / 2.0]
    };
    let red = [1.0, 0.0, 0.0, 1.0];
    let blue = [0.0, 0.0, 1.0, 1.0];
    let paints = [
        gradient(
            across(0),
            StopEncoding::SRGB,
            false,
            vec![stop(red, 0.0), stop(blue, 1.0)],
        ),
        gradient(
            across(1),
            StopEncoding::OKLAB,
            false,
            vec![
                stop([0.628, 0.2249, 0.1258, 1.0], 0.0),
                stop([0.452, -0.0325, -0.3115, 1.0], 1.0),
            ],
        ),
        gradient(
            across(2),
            StopEncoding::LINEAR_SRGB,
            false,
            vec![
                stop([0.0, 0.0, 0.0, 1.0], 0.0),
                stop([1.0, 1.0, 1.0, 1.0], 1.0),
            ],
        ),
        gradient(
            GradientShape::Radial {
                center: centre(3),
                radius: [18.0, 12.0],
            },
            StopEncoding::SRGB,
            false,
            vec![
                stop([1.0, 1.0, 0.0, 1.0], 0.0),
                stop([0.0, 0.5, 0.0, 1.0], 1.0),
            ],
        ),
        gradient(
            GradientShape::Conic {
                center: centre(4),
                angle: 0.5,
            },
            StopEncoding::SRGB,
            false,
            vec![
                stop(red, 0.0),
                stop([0.0, 1.0, 0.0, 1.0], 0.5),
                stop(blue, 1.0),
            ],
        ),
        gradient(
            {
                let [x, y, _, _] = cell(5);
                GradientShape::Linear {
                    start: [x, y],
                    end: [x + 9.0, y + 9.0],
                }
            },
            StopEncoding::SRGB,
            true,
            vec![stop(red, 0.0), stop(blue, 1.0)],
        ),
        gradient(
            across(6),
            StopEncoding::SRGB,
            false,
            vec![
                stop(red, 0.0),
                stop(red, 0.5),
                stop(blue, 0.5),
                stop(blue, 1.0),
            ],
        ),
        gradient(
            across(7),
            StopEncoding::SRGB,
            false,
            vec![
                stop([0.5, 0.0, 0.0, 0.5], 0.0),
                stop([0.0, 0.0, 0.0, 0.0], 1.0),
            ],
        ),
        gradient(
            across(8),
            StopEncoding::OKLAB,
            false,
            vec![
                stop([0.628, 0.2249, 0.1258, 1.0], 0.0),
                stop([0.0, 0.0, 0.0, 0.0], 1.0),
            ],
        ),
    ];
    for (index, paint) in paints.into_iter().enumerate() {
        let paint = builder.paint(paint);
        builder.quad(filled(cell(index), paint));
    }
    golden("gradients", &canvas.draw(builder.finish()), RAMPS);
}
