//! Polyline segments and their caps.

use iceblink::MarkFlags;

use crate::math::edge::Distance;
use crate::math::vec::length;

const CAP_SQUARE: u32 = 1;
const CAP_ROUND: u32 = 2;

/// Reports whether a payload vertex separates runs.
pub(crate) fn is_separator(vertex: [f32; 2]) -> bool {
    vertex[0].is_nan() || vertex[1].is_nan()
}

/// Returns the caps of the segment from vertex `at` to vertex `at + 1`, the start cap in bits 0 and
/// 1 and the end cap in bits 2 and 3.
///
/// An end beside a separator takes the item's cap, and every other end is round.
pub(crate) fn segment_caps(vertices: &[[f32; 2]], at: usize, flags: MarkFlags) -> u32 {
    let separator = |index: Option<usize>| {
        index
            .and_then(|index| vertices.get(index))
            .is_none_or(|vertex| is_separator(*vertex))
    };
    let start = if separator(at.checked_sub(1)) {
        flags.start_cap()
    } else {
        CAP_ROUND
    };
    let end = if separator(Some(at + 2)) {
        flags.end_cap()
    } else {
        CAP_ROUND
    };
    start | (end << 2)
}

/// The distance-space quad a segment draws over: the segment's start, direction and normal, and
/// how far the quad extends past the segment.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SegmentQuad {
    a: [f32; 2],
    direction: [f32; 2],
    normal: [f32; 2],
    length: f32,
    reach: f32,
}

impl SegmentQuad {
    /// Returns the quad of the segment from `a` to `b`, in distance space.
    pub(crate) fn new(a: [f32; 2], b: [f32; 2], half_width: f32, margin: f32) -> Self {
        let along = [b[0] - a[0], b[1] - a[1]];
        let length = length(along);
        let direction = if length > 0.0 {
            [along[0] / length, along[1] / length]
        } else {
            [1.0, 0.0]
        };
        Self {
            a,
            direction,
            normal: [-direction[1], direction[0]],
            length,
            reach: half_width + margin,
        }
    }

    /// Returns the quad's four corners.
    pub(crate) fn corners(&self) -> [[f32; 2]; 4] {
        let at = |u: f32, v: f32| {
            [
                self.a[0] + self.direction[0] * u + self.normal[0] * v,
                self.a[1] + self.direction[1] * u + self.normal[1] * v,
            ]
        };
        let r = self.reach;
        [
            at(-r, -r),
            at(self.length + r, -r),
            at(-r, r),
            at(self.length + r, r),
        ]
    }

    /// Reports whether the quad holds `point`.
    pub(crate) fn holds(&self, point: [f32; 2]) -> bool {
        let from = [point[0] - self.a[0], point[1] - self.a[1]];
        let u = from[0] * self.direction[0] + from[1] * self.direction[1];
        let v = from[0] * self.normal[0] + from[1] * self.normal[1];
        u >= -self.reach && u <= self.length + self.reach && v.abs() <= self.reach
    }
}

/// Returns the distance to the segment from `a` to `b` stroked `half` wide on each side.
pub(crate) fn segment_value(
    point: [f32; 2],
    a: [f32; 2],
    b: [f32; 2],
    half: f32,
    caps: u32,
) -> Distance {
    let along = [b[0] - a[0], b[1] - a[1]];
    let length_along = length(along);
    let direction = if length_along > 0.0 {
        [along[0] / length_along, along[1] / length_along]
    } else {
        [1.0, 0.0]
    };
    let normal = [-direction[1], direction[0]];
    let from = [point[0] - a[0], point[1] - a[1]];
    let u = from[0] * direction[0] + from[1] * direction[1];
    let v = from[0] * normal[0] + from[1] * normal[1];
    let start = caps & 3;
    let end = (caps >> 2) & 3;
    let reach_start = if start == CAP_SQUARE { half } else { 0.0 };
    let reach_end = if end == CAP_SQUARE { half } else { 0.0 };
    let lengthwise = reach_start + length_along + reach_end;
    let before = Distance {
        d: -reach_start - u,
        gradient: [-direction[0], -direction[1]],
        band: lengthwise,
    };
    let after = Distance {
        d: u - (length_along + reach_end),
        gradient: direction,
        band: lengthwise,
    };
    let side = if v >= 0.0 { 1.0 } else { -1.0 };
    let beside = Distance {
        d: v.abs() - half,
        gradient: [normal[0] * side, normal[1] * side],
        band: 2.0 * half,
    };
    let mut out = before.max(after).max(beside);
    if start == CAP_ROUND {
        let mut cap = Distance::radial(point, a, half);
        cap.band = 2.0 * half;
        out = out.min(cap);
    }
    if end == CAP_ROUND {
        let mut cap = Distance::radial(point, b, half);
        cap.band = 2.0 * half;
        out = out.min(cap);
    }
    out
}
