//! The coverage of discs, boxes and polyline segments at a sample, and the quads they draw.

mod boxes;
mod disc;
mod segment;

pub(super) use self::boxes::{box_coverage, box_quad};
pub(super) use self::disc::{disc_coverage, disc_quad};
pub(super) use self::segment::{SegmentQuad, is_separator, segment_caps, segment_value};

/// Returns the corners of the axis-aligned distance-space box from `low` to `high`.
pub(super) fn corners(low: [f32; 2], high: [f32; 2]) -> [[f32; 2]; 4] {
    [low, [high[0], low[1]], [low[0], high[1]], high]
}

/// Reports whether `point` lies in the box with corners `a` and `b`, in either order.
pub(super) fn inside(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> bool {
    (0..2).all(|axis| point[axis] >= a[axis].min(b[axis]) && point[axis] <= a[axis].max(b[axis]))
}
