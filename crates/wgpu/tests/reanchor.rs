//! A ramp moves with the quad it paints when a resident chunk is drawn at an offset.
//!
//! `resident.rs` tests the linear fill. These tests cover the two other ways a quad takes a ramp:
//! along its border, and as a radial fill, which a displacement moves in a way a linear ramp would
//! not show.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, rect};
use common::device::{plain_renderer, present};
use iceblink::{
    FrameGradient, FramePaint, FrameStop, GradientShape, LoweredChunk, PaintRef, Quad, StopEncoding,
};
use iceblink_wgpu::Pixels;

const LEFT: f32 = 16.0;
const WIDTH: f32 = 96.0;
const HEIGHT: f32 = 40.0;
/// Where the quad is recorded.
const TOP: f32 = 8.0;
/// How far the chunk is moved, more than the quad's height so a ramp left behind runs off its end.
const STEP: f32 = 64.0;

/// The ways a quad under test takes its ramp.
#[derive(Clone, Copy, Debug)]
enum Arm {
    /// A border painted with a linear ramp, and no fill.
    Stroke,
    /// A fill with a radial ramp.
    Radial,
}

/// Returns a ramp across the quad, in the quad's paint coordinates.
fn ramp(radial: bool) -> FramePaint {
    let shape = if radial {
        GradientShape::Radial {
            center: [WIDTH * 0.5, HEIGHT * 0.5],
            radius: [WIDTH * 0.5, HEIGHT * 0.5],
        }
    } else {
        GradientShape::Linear {
            start: [0.0, 0.0],
            end: [WIDTH, HEIGHT],
        }
    };
    // Three colours far enough apart that sampling the wrong part of the ramp is a difference no
    // rounding could make.
    let stop = |offset: f32, [r, g, b]: [u8; 3]| FrameStop {
        color: [r, g, b, 255].map(|channel| f32::from(channel) / 255.0),
        offset,
    };
    FramePaint::Gradient(FrameGradient {
        shape,
        encoding: StopEncoding::SRGB,
        repeat: false,
        stops: Arc::from(vec![
            stop(0.0, [126, 227, 255]),
            stop(0.55, [47, 107, 255]),
            stop(1.0, [209, 139, 255]),
        ]),
    })
}

/// Returns `arm`'s quad with its top edge at `top`, painted by `paint`.
fn quad(arm: Arm, top: f32, paint: PaintRef) -> Quad {
    let mut quad = match arm {
        Arm::Radial => filled(rect(LEFT, top, WIDTH, HEIGHT), paint),
        Arm::Stroke => {
            let mut quad = filled(rect(LEFT, top, WIDTH, HEIGHT), PaintRef::NONE);
            quad.border = [10.0; 4];
            quad.stroke = paint;
            quad
        }
    };
    quad.paint_origin = [LEFT, top];
    quad
}

/// Draws `arm` recorded at its travelled position, with nothing resident.
fn repainted(arm: Arm) -> Pixels {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let paint = builder.paint(ramp(matches!(arm, Arm::Radial)));
    builder.quad(quad(arm, TOP + STEP, paint));
    present(&mut renderer, &builder.finish())
}

/// Draws `arm` from a resident chunk recorded at [`TOP`] and drawn [`STEP`] further down.
fn moved(arm: Arm) -> Pixels {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let paint = builder.paint(ramp(matches!(arm, Arm::Radial)));
    let chunk = Arc::new(LoweredChunk {
        quads: vec![quad(arm, TOP, paint)],
        ..LoweredChunk::default()
    });
    builder.insert(1, Arc::clone(&chunk));
    builder.draw_resident_quads(1, [0.0, STEP], &chunk.quads);
    present(&mut renderer, &builder.finish())
}

/// Asserts that the moved quad of `arm` in `picture` changes colour across its width.
// A box that is one colour across its width would pass the comparison whatever the sampler does.
fn varies(picture: &Pixels, arm: Arm) {
    let y = (TOP
        + STEP
        + if matches!(arm, Arm::Stroke) {
            4.0
        } else {
            HEIGHT * 0.5
        }) as i32;
    assert_ne!(
        picture.rgba(LEFT as i32 + 6, y),
        picture.rgba((LEFT + WIDTH) as i32 - 6, y),
        "{arm:?}: the repainted quad is one colour across its width"
    );
}

#[test]
fn a_moved_ramp_along_a_border_and_a_moved_radial_ramp_draw_their_repaint() {
    for arm in [Arm::Stroke, Arm::Radial] {
        let whole = repainted(arm);
        varies(&whole, arm);
        assert_eq!(
            moved(arm).max_difference(&whole),
            0,
            "{arm:?}: a quad drawn at an offset differs from the quad recorded there"
        );
    }
}
