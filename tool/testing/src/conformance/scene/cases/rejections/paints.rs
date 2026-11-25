//! Paint kinds: a slot keeps its kind, and a reference refers to a paint of its kind.

use std::sync::Arc;

use iceblink::color::{Color as StopColor, ColorSpace};
use iceblink::{
    Color, Gradient, GradientShape, GradientStop, Interpolation, Paint, PaintKind, RejectReason,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::key;
use crate::conformance::scene::cases::rejections::{quad_with, rejects};
use crate::conformance::scene::stage::Stage;

/// A paint slot keeps its kind, and a reference refers to a paint of its kind.
pub fn paint_kinds(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    stage.commit(tx);

    let mut tx = stage.tx();
    tx.set_paint(paint, ramp());
    rejects(
        &mut stage,
        tx,
        RejectReason::PaintKindChanged,
        "a solid slot rewritten as a gradient",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), stage.props());
    let fill = chunk.paint_ref(PaintKind::GRADIENT, paint);
    chunk.quad(quad_with(fill));
    rejects(
        &mut stage,
        tx,
        RejectReason::PaintKindChanged,
        "a reference of another kind than its paint",
    );
}

/// A linear gradient from red to blue.
fn ramp() -> Paint {
    Paint::Gradient(Gradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [8.0, 0.0],
        },
        stops: Arc::from(vec![
            GradientStop::new(0.0, StopColor::new(ColorSpace::Srgb, [1.0, 0.0, 0.0], 1.0)),
            GradientStop::new(1.0, StopColor::new(ColorSpace::Srgb, [0.0, 0.0, 1.0], 1.0)),
        ]),
        interpolation: Interpolation::new(ColorSpace::Srgb),
        repeat: false,
    })
}
