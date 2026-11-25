//! Paths mapped through a projective matrix.

use iceblink::geom::Matrix4;
use kurbo::{Affine, BezPath, PathEl, Point};

/// Returns `path` placed by `placement` and then mapped through `world` onto the device.
///
/// Control points are mapped one by one, so curves stay curves of the same kind.
pub(crate) fn project(path: &BezPath, placement: Affine, world: &Matrix4) -> BezPath {
    let map = |point: Point| {
        let local = placement * point;
        let [x, y, _] = world.transform_point(local.x as f32, local.y as f32, 0.0);
        Point::new(f64::from(x), f64::from(y))
    };
    path.elements()
        .iter()
        .map(|element| match *element {
            PathEl::MoveTo(p) => PathEl::MoveTo(map(p)),
            PathEl::LineTo(p) => PathEl::LineTo(map(p)),
            PathEl::QuadTo(a, b) => PathEl::QuadTo(map(a), map(b)),
            PathEl::CurveTo(a, b, c) => PathEl::CurveTo(map(a), map(b), map(c)),
            PathEl::ClosePath => PathEl::ClosePath,
        })
        .collect()
}

/// Reports whether every point of `path`, placed by `placement`, maps through `world` to a
/// homogeneous `W` above zero.
pub(crate) fn in_front(path: &BezPath, placement: Affine, world: &Matrix4) -> bool {
    let front = |point: &Point| {
        let local = placement * *point;
        let [_, _, _, w] = world.transform_vector4([local.x as f32, local.y as f32, 0.0, 1.0]);
        w > 0.0
    };
    path.elements().iter().all(|element| match element {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => front(p),
        PathEl::QuadTo(a, b) => front(a) && front(b),
        PathEl::CurveTo(a, b, c) => front(a) && front(b) && front(c),
        PathEl::ClosePath => true,
    })
}
