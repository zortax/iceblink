//! Signed distances with gradients, and the exact pixel area on the inner side of an edge.

use crate::math::vec::{dot, length};

/// A signed distance in distance-space units, negative inside, and its gradient.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Distance {
    pub(crate) d: f32,
    pub(crate) gradient: [f32; 2],
    /// The width of the shape behind the edge, or zero for a shape with no far edge.
    pub(crate) band: f32,
}

impl Distance {
    /// Returns the distance from a disc of `radius` around `centre`.
    pub(crate) fn radial(point: [f32; 2], centre: [f32; 2], radius: f32) -> Self {
        let along = [point[0] - centre[0], point[1] - centre[1]];
        let length_along = length(along);
        let gradient = if length_along > 0.0 {
            [along[0] / length_along, along[1] / length_along]
        } else {
            [1.0, 0.0]
        };
        Self {
            d: length_along - radius,
            gradient,
            band: 0.0,
        }
    }

    /// Returns the larger of two distances.
    pub(crate) fn max(self, other: Self) -> Self {
        if other.d > self.d { other } else { self }
    }

    /// Returns the smaller of two distances.
    pub(crate) fn min(self, other: Self) -> Self {
        if other.d < self.d { other } else { self }
    }
}

/// Returns the area of the pixel inside `distance`, where `across` and `down` are how far one
/// device pixel moves the distance-space point.
///
/// A band thinner than a pixel loses the area past its far edge.
pub(crate) fn sdf_coverage(distance: Distance, across: [f32; 2], down: [f32; 2]) -> f32 {
    let normal = [dot(distance.gradient, across), dot(distance.gradient, down)];
    let step = length(normal).max(1.0e-6);
    let along = [normal[0].abs() / step, normal[1].abs() / step];
    let a = along[0].max(along[1]);
    let b = along[0].min(along[1]);
    let inside = -distance.d / step;
    let near = edge_coverage(inside, a, b);
    if distance.band <= 0.0 {
        return near;
    }
    (near - edge_coverage(inside - distance.band / step, a, b)).max(0.0)
}

/// Returns the area of a unit pixel on the inner side of a straight edge `inside` pixels from its
/// centre, for an edge whose unit normal has the components `a` and `b`, `a` the larger.
pub(crate) fn edge_coverage(inside: f32, a: f32, b: f32) -> f32 {
    let outer = 0.5 * (a + b);
    let inner = 0.5 * (a - b);
    if inside <= -outer {
        return 0.0;
    }
    if inside >= outer {
        return 1.0;
    }
    if b < 1.0e-4 {
        return (0.5 + inside / a).clamp(0.0, 1.0);
    }
    if inside < -inner {
        let t = inside + outer;
        return t * t / (2.0 * a * b);
    }
    if inside > inner {
        let t = outer - inside;
        return 1.0 - t * t / (2.0 * a * b);
    }
    0.5 + inside / a
}
