//! Rejection of non-finite numbers and invalid colours.

use iceblink::kurbo::{Affine, BezPath, PathEl, Stroke};
use iceblink::{Color, Matrix4, RejectReason};

/// Fails with `InvalidItem` unless every number is finite.
pub(crate) fn numbers(values: &[f32]) -> Result<(), RejectReason> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(RejectReason::InvalidItem)
    }
}

/// How far a colour channel may lie above its alpha, for premultiplying that rounds up.
const PREMULTIPLIED_SLACK: f32 = 1.0e-4;

/// Fails with `InvalidItem` unless the colour is premultiplied: every component in `[0, 1]`, and
/// no colour channel more than [`PREMULTIPLIED_SLACK`] above its alpha.
pub(crate) fn color(color: Color) -> Result<(), RejectReason> {
    let [r, g, b, a] = color.0;
    let valid = color
        .0
        .iter()
        .all(|component| (0.0..=1.0).contains(component))
        && [r, g, b]
            .iter()
            .all(|channel| *channel <= a + PREMULTIPLIED_SLACK);
    if valid {
        Ok(())
    } else {
        Err(RejectReason::InvalidItem)
    }
}

pub(crate) fn matrix(matrix: &Matrix4) -> Result<(), RejectReason> {
    numbers(matrix.columns.as_flattened())
}

pub(crate) fn affine(affine: Affine) -> Result<(), RejectReason> {
    wide(&affine.as_coeffs())
}

pub(crate) fn path(path: &BezPath) -> Result<(), RejectReason> {
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => wide(&[p.x, p.y])?,
            PathEl::QuadTo(a, b) => wide(&[a.x, a.y, b.x, b.y])?,
            PathEl::CurveTo(a, b, c) => wide(&[a.x, a.y, b.x, b.y, c.x, c.y])?,
            PathEl::ClosePath => {}
        }
    }

    Ok(())
}

pub(crate) fn stroke(stroke: &Stroke) -> Result<(), RejectReason> {
    wide(&[stroke.width, stroke.miter_limit, stroke.dash_offset])?;
    wide(&stroke.dash_pattern)
}

fn wide(values: &[f64]) -> Result<(), RejectReason> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(RejectReason::InvalidItem)
    }
}
