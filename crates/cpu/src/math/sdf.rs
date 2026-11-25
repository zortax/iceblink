// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// Ported from iceblink-wgsl's `sdf.wgsl`. The signed distance function, the quadrant-based corner
// selection and the half-pixel coverage rule are adapted from that work, which is licensed under
// the Apache License, Version 2.0, and have been modified: the scalar corner radius is generalised
// to a pair of elliptical semi-axes per corner with a superellipse exponent per corner, and the
// circular case is kept as the exact fast path.

//! The rounded-rectangle signed distance, negative inside.

use crate::math::saturate;
use crate::math::vec::length;

/// The exponent of an elliptical corner.
pub(crate) const CORNER_ROUND: f32 = 2.0;

/// Returns the radii of the corner whose quadrant `center_to_point` lies in.
pub(crate) fn pick_corner_radii(center_to_point: [f32; 2], radii: &[f32; 8]) -> [f32; 2] {
    if center_to_point[0] < 0.0 {
        if center_to_point[1] < 0.0 {
            return [radii[0], radii[1]];
        }
        return [radii[6], radii[7]];
    }
    if center_to_point[1] < 0.0 {
        return [radii[2], radii[3]];
    }
    [radii[4], radii[5]]
}

/// Returns the exponent of the corner whose quadrant `center_to_point` lies in.
pub(crate) fn pick_corner_shape(center_to_point: [f32; 2], shapes: &[f32; 4]) -> f32 {
    if center_to_point[0] < 0.0 {
        if center_to_point[1] < 0.0 {
            shapes[0]
        } else {
            shapes[3]
        }
    } else if center_to_point[1] < 0.0 {
        shapes[1]
    } else {
        shapes[2]
    }
}

/// Returns the first-order distance to a superellipse corner, for a point mirrored into the
/// bottom-right quadrant.
pub(crate) fn superellipse_corner_sdf(point: [f32; 2], r: [f32; 2], n: f32) -> f32 {
    let power = n.clamp(0.01, 64.0);
    let normalised = [
        (point[0].abs() / r[0]).max(1.0e-6),
        (point[1].abs() / r[1]).max(1.0e-6),
    ];
    let raised = [normalised[0].powf(power), normalised[1].powf(power)];
    let sum = (raised[0] + raised[1]).max(1.0e-12);
    let value = sum.powf(1.0 / power);
    let slope = [
        normalised[0].powf(power - 1.0) / r[0],
        normalised[1].powf(power - 1.0) / r[1],
    ];
    let gradient = sum.powf(1.0 / power - 1.0) * length(slope);
    (value - 1.0) / gradient.max(1.0e-6)
}

/// Returns the distance to the rounded boundary for a point measured from the corner's centre and
/// mirrored into the bottom-right quadrant.
pub(crate) fn quad_sdf_impl(corner_center_to_point: [f32; 2], r: [f32; 2], shape: f32) -> f32 {
    let [x, y] = corner_center_to_point;
    let inside_corner = x > 0.0 && y > 0.0;
    if !inside_corner || r[0] <= 0.0 || r[1] <= 0.0 {
        return (x - r[0]).max(y - r[1]);
    }
    if shape != CORNER_ROUND {
        return superellipse_corner_sdf(corner_center_to_point, r, shape);
    }
    if r[0] == r[1] {
        return length(corner_center_to_point) - r[0];
    }
    let k1 = length([x / r[0], y / r[1]]);
    let k2 = length([x / (r[0] * r[0]), y / (r[1] * r[1])]);
    k1 * (k1 - 1.0) / k2
}

/// Returns the distance from `point` to the rounded rectangle `bounds` (`[x, y, w, h]`).
pub(crate) fn quad_sdf(
    point: [f32; 2],
    bounds: [f32; 4],
    radii: &[f32; 8],
    shapes: &[f32; 4],
) -> f32 {
    let half = [bounds[2] * 0.5, bounds[3] * 0.5];
    let center = [bounds[0] + half[0], bounds[1] + half[1]];
    let center_to_point = [point[0] - center[0], point[1] - center[1]];
    let r = pick_corner_radii(center_to_point, radii);
    let shape = pick_corner_shape(center_to_point, shapes);
    let corner_to_point = [
        center_to_point[0].abs() - half[0],
        center_to_point[1].abs() - half[1],
    ];
    quad_sdf_impl(
        [corner_to_point[0] + r[0], corner_to_point[1] + r[1]],
        r,
        shape,
    )
}

/// Returns the distance to the inner edge of a border corner, positive inside it.
pub(crate) fn quarter_ellipse_sdf(point: [f32; 2], radii: [f32; 2], shape: f32) -> f32 {
    if radii[0] <= 0.0 || radii[1] <= 0.0 {
        return -(point[0] - radii[0]).max(point[1] - radii[1]);
    }
    if shape != CORNER_ROUND {
        return -superellipse_corner_sdf(point, radii, shape);
    }
    let k1 = length([point[0] / radii[0], point[1] / radii[1]]);
    let k2 = length([
        point[0] / (radii[0] * radii[0]),
        point[1] / (radii[1] * radii[1]),
    ]);
    -(k1 * (k1 - 1.0) / k2)
}

/// Returns the coverage of `point` by a rounded rectangle, with a band of half a unit on either
/// side.
pub(crate) fn rect_coverage(
    point: [f32; 2],
    bounds: [f32; 4],
    radii: &[f32; 8],
    shapes: &[f32; 4],
) -> f32 {
    saturate(0.5 - quad_sdf(point, bounds, radii, shapes))
}

/// Reports whether `point` lies outside `bounds`, whose edges are inside.
pub(crate) fn outside_bounds(point: [f32; 2], bounds: [f32; 4]) -> bool {
    point[0] < bounds[0]
        || point[1] < bounds[1]
        || point[0] > bounds[0] + bounds[2]
        || point[1] > bounds[1] + bounds[3]
}
