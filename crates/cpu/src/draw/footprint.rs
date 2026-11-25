//! The device pixels a rectangle of a space can cover.

use iceblink::geom::{Device, Point, Rect};

use crate::math::Floor as _;
use crate::tables::Space;

/// Returns the pixels of `within` whose centres the local rectangle `rect` of `space` can hold, as
/// [`footprint_of`] does for its corners.
pub(crate) fn footprint(
    space: &Space,
    rect: [f32; 4],
    within: Rect<i32, Device>,
) -> Option<Rect<i32, Device>> {
    let [x, y, width, height] = rect;
    let corners = [
        [x, y],
        [x + width, y],
        [x, y + height],
        [x + width, y + height],
    ];
    footprint_of(space, corners, within)
}

/// Returns the pixels of `within` whose centres the local quadrilateral `corners` of `space` can
/// hold.
///
/// A quadrilateral with a corner behind the viewer can hold every pixel of `within`.
///
/// Returns `None` when the footprint misses `within` or is not finite.
pub(crate) fn footprint_of(
    space: &Space,
    corners: [[f32; 2]; 4],
    within: Rect<i32, Device>,
) -> Option<Rect<i32, Device>> {
    let mut low = [f32::INFINITY; 2];
    let mut high = [f32::NEG_INFINITY; 2];
    for corner in corners {
        let Some(device) = space.device(corner) else {
            return Some(within);
        };
        for axis in 0..2 {
            low[axis] = low[axis].min(device[axis]);
            high[axis] = high[axis].max(device[axis]);
        }
    }
    if !(low[0].is_finite() && low[1].is_finite() && high[0].is_finite() && high[1].is_finite()) {
        return None;
    }

    let clamp = |value: f32| value.clamp(-1.0e9, 1.0e9) as i32;
    let reached = Rect::from_corners(
        Point::new(clamp(low[0].floored()), clamp(low[1].floored())),
        Point::new(clamp(high[0].ceil()), clamp(high[1].ceil())),
    );
    reached.intersection(within).filter(|cut| !cut.is_empty())
}

/// Reports whether the local point `point` lies in `rect`, edges included.
pub(crate) fn holds(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[1] >= rect[1]
        && point[0] <= rect[0] + rect[2]
        && point[1] <= rect[1] + rect[3]
}
