//! Outlines under projective matrices.

use iceblink::Matrix4;
use kurbo::{BezPath, PathEl, Point};

/// How far the flattened outline strays from its curves, in the outline's own units.
const TOLERANCE: f64 = 0.05;

/// Returns `path` flattened and mapped through `matrix`, with the perspective divide.
pub(super) fn projected(path: &BezPath, matrix: &Matrix4) -> BezPath {
    let map = |point: Point| {
        let [x, y, _] = matrix.transform_point(point.x as f32, point.y as f32, 0.0);
        Point::new(f64::from(x), f64::from(y))
    };
    let mut out = BezPath::new();
    kurbo::flatten(path.iter(), TOLERANCE, |element| match element {
        PathEl::MoveTo(point) => out.move_to(map(point)),
        PathEl::LineTo(point) => out.line_to(map(point)),
        PathEl::ClosePath => out.close_path(),
        // Flattening yields only the three elements above.
        PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
    });
    out
}
