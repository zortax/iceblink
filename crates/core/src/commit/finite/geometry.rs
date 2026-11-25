//! Path geometry and stroke styles.

use kurbo::{BezPath, PathEl, Point, Stroke};

/// Reports whether every point of `path` is finite.
pub(crate) fn path(path: &BezPath) -> bool {
    let finite = |point: &Point| point.x.is_finite() && point.y.is_finite();
    path.elements().iter().all(|element| match element {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => finite(p),
        PathEl::QuadTo(a, b) => finite(a) && finite(b),
        PathEl::CurveTo(a, b, c) => finite(a) && finite(b) && finite(c),
        PathEl::ClosePath => true,
    })
}

/// Reports whether the width, miter limit and dashes of `style` are finite.
pub(crate) fn stroke(style: &Stroke) -> bool {
    [style.width, style.miter_limit, style.dash_offset]
        .iter()
        .chain(style.dash_pattern.iter())
        .all(|value| value.is_finite())
}
