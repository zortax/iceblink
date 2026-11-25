//! The dash coverage of a dashed or dotted border.

use iceblink::{BorderStyle, Quad};

use crate::math::{fmod, saturate};

const PI: f32 = core::f32::consts::PI;

/// A point of a quad's border, measured as the quad's shading measures it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Spot {
    /// The local point.
    pub(super) point: [f32; 2],
    /// The point from the quad's centre.
    pub(super) center_to_point: [f32; 2],
    /// The point from the centre of its corner, mirrored into the bottom-right quadrant.
    pub(super) corner_center: [f32; 2],
    /// The radii of the point's corner.
    pub(super) corner_radii: [f32; 2],
    /// Whether the point lies in the zone of a rounded corner.
    pub(super) near_rounded: bool,
    /// Whether the quad has no radii.
    pub(super) unrounded: bool,
    /// Half a device pixel, in local units.
    pub(super) threshold: f32,
}

/// Returns how much of the pixel at the spot the dashes of `quad`'s border cover.
///
/// Dashes are `2w` long for a dashed border and `w` for a dotted one, with gaps of `w`.
pub(super) fn coverage(quad: &Quad, at: &Spot) -> f32 {
    let dash_per_width = if quad.style == BorderStyle::DOTTED {
        1.0
    } else {
        2.0
    };
    let period_per_width = dash_per_width + 1.0;
    let numerator = 1.0 / period_per_width;
    let Position {
        t,
        mut max_t,
        velocity,
    } = if at.unrounded {
        along_side(quad, at, numerator)
    } else {
        along_perimeter(quad, at, numerator)
    };

    let dash_length = dash_per_width / period_per_width;
    if at.unrounded {
        // A straight run starts and ends with a dash.
        max_t -= dash_length;
    }
    if max_t >= 1.0 {
        let count = max_t.floor();
        return alpha(t, max_t / count, dash_length, velocity, at.threshold);
    }
    if at.unrounded {
        let gap = max_t - dash_length;
        if gap > 0.0 {
            return alpha(t, dash_length + gap, dash_length, velocity, at.threshold);
        }
    }
    1.0
}

// A spot's place along its border, in periods of the dash pattern.
struct Position {
    t: f32,
    // The length of the run the spot lies on.
    max_t: f32,
    // Periods per local unit.
    velocity: f32,
}

// The position of a spot of a quad with no radii. Each side lays its dashes out on its own, so
// every side starts and ends with a dash.
fn along_side(quad: &Quad, at: &Spot, numerator: f32) -> Position {
    let [top, right, bottom, left] = quad.border;
    let size = [quad.bounds[2], quad.bounds[3]];
    let local = [at.point[0] - quad.bounds[0], at.point[1] - quad.bounds[1]];
    let horizontal = at.corner_center[0] < at.corner_center[1];
    let width = if horizontal {
        bottom.max(top)
    } else {
        right.max(left)
    };
    let velocity = if width > 0.0 { numerator / width } else { 0.0 };
    Position {
        t: if horizontal { local[0] } else { local[1] } * velocity,
        max_t: if horizontal { size[0] } else { size[1] } * velocity,
        velocity,
    }
}

// The position of a spot of a rounded quad, whose dashes run clockwise around the whole perimeter.
fn along_perimeter(quad: &Quad, at: &Spot, numerator: f32) -> Position {
    let Spot {
        point,
        center_to_point,
        corner_center,
        corner_radii,
        near_rounded,
        ..
    } = *at;
    let [top, right, bottom, left] = quad.border;
    let size = [quad.bounds[2], quad.bounds[3]];
    let local = [point[0] - quad.bounds[0], point[1] - quad.bounds[1]];

    let r = &quad.radii;
    let (r_tl, r_tr, r_br, r_bl) = ([r[0], r[1]], [r[2], r[3]], [r[4], r[5]], [r[6], r[7]]);
    let rate = |width: f32| if width > 0.0 { numerator / width } else { 0.0 };
    let (dv_t, dv_r, dv_b, dv_l) = (rate(top), rate(right), rate(bottom), rate(left));
    let s_t = (size[0] - r_tl[0] - r_tr[0]).max(0.0) * dv_t;
    let s_r = (size[1] - r_tr[1] - r_br[1]).max(0.0) * dv_r;
    let s_b = (size[0] - r_br[0] - r_bl[0]).max(0.0) * dv_b;
    let s_l = (size[1] - r_bl[1] - r_tl[1]).max(0.0) * dv_l;
    let cv_tr = corner_velocity(dv_t, dv_r);
    let cv_br = corner_velocity(dv_b, dv_r);
    let cv_bl = corner_velocity(dv_b, dv_l);
    let cv_tl = corner_velocity(dv_t, dv_l);
    let c_tr = quarter_ellipse_perimeter(r_tr) * cv_tr;
    let c_br = quarter_ellipse_perimeter(r_br) * cv_br;
    let c_bl = quarter_ellipse_perimeter(r_bl) * cv_bl;
    let c_tl = quarter_ellipse_perimeter(r_tl) * cv_tl;
    let upto_tr = s_t;
    let upto_r = upto_tr + c_tr;
    let upto_br = upto_r + s_r;
    let upto_b = upto_br + c_br;
    let upto_bl = upto_b + s_b;
    let upto_l = upto_bl + c_bl;
    let upto_tl = upto_l + s_l;
    let max_t = upto_tl + c_tl;

    let (t, velocity);
    if near_rounded {
        // The eccentric anomaly keeps the dash rhythm continuous across an elliptical corner.
        let radii = [corner_radii[0].max(1e-6), corner_radii[1].max(1e-6)];
        let anomaly = (corner_center[1] / radii[1]).atan2(corner_center[0] / radii[0]);
        let fraction = saturate(anomaly / (0.5 * PI));
        if center_to_point[0] >= 0.0 {
            if center_to_point[1] < 0.0 {
                velocity = cv_tr;
                t = upto_r - fraction * c_tr;
            } else {
                velocity = cv_br;
                t = upto_br + fraction * c_br;
            }
        } else if center_to_point[1] >= 0.0 {
            velocity = cv_bl;
            t = upto_l - fraction * c_bl;
        } else {
            velocity = cv_tl;
            t = upto_tl + fraction * c_tl;
        }
    } else if corner_center[0] < corner_center[1] {
        if center_to_point[1] < 0.0 {
            velocity = dv_t;
            t = (local[0] - r_tl[0]) * velocity;
        } else {
            velocity = dv_b;
            t = upto_bl - (local[0] - r_bl[0]) * velocity;
        }
    } else if center_to_point[0] < 0.0 {
        velocity = dv_l;
        t = upto_tl - (local[1] - r_tl[1]) * velocity;
    } else {
        velocity = dv_r;
        t = upto_r + (local[1] - r_tr[1]) * velocity;
    }
    Position { t, max_t, velocity }
}

// Ramanujan's quarter-ellipse arc length.
fn quarter_ellipse_perimeter(r: [f32; 2]) -> f32 {
    if r[0] <= 0.0 && r[1] <= 0.0 {
        return 0.0;
    }
    let h = ((r[0] - r[1]) * (r[0] - r[1])) / ((r[0] + r[1]) * (r[0] + r[1])).max(1e-6);
    0.25 * PI * (r[0] + r[1]) * (1.0 + (3.0 * h) / (10.0 + (4.0 - 3.0 * h).max(0.0).sqrt()))
}

// The slower of two velocities, where zero means a side of zero width.
fn corner_velocity(first: f32, second: f32) -> f32 {
    if first == 0.0 {
        return second;
    }
    if second == 0.0 {
        return first;
    }
    first.min(second)
}

// The coverage of a dash at dash-space position `t`.
fn alpha(t: f32, period: f32, length: f32, velocity: f32, threshold: f32) -> f32 {
    let half_period = period * 0.5;
    let half_length = length * 0.5;
    let centered = fmod(t + half_period - half_length, period) - half_period;
    let distance = centered.abs() - half_length;
    saturate(threshold - distance / velocity.max(1e-6))
}
