//! The device pixels a path can change.

use iceblink::geom::{Point, Rect, Size};
use iceblink::{DeviceRect, PathStroke};
use kurbo::{Affine, Cap, Join};

/// Returns the whole device pixels that cover `bounds` mapped through `transform`.
///
/// A rectangle with a non-finite edge covers nothing.
pub(crate) fn device_ink(bounds: kurbo::Rect, transform: Affine) -> DeviceRect {
    let mapped = transform.transform_rect_bbox(bounds);
    let edges = [mapped.x0, mapped.y0, mapped.x1, mapped.y1];
    if !edges.iter().all(|edge| edge.is_finite()) || mapped.width() < 0.0 || mapped.height() < 0.0 {
        return Rect::new(Point::new(0, 0), Size::new(0, 0));
    }

    let clamp = |value: f64| value.clamp(f64::from(i32::MIN / 2), f64::from(i32::MAX / 2)) as i32;
    let left = clamp(mapped.x0.floor());
    let top = clamp(mapped.y0.floor());
    let right = clamp(mapped.x1.ceil());
    let bottom = clamp(mapped.y1.ceil());
    Rect::new(Point::new(left, top), Size::new(right - left, bottom - top))
}

/// Returns how far a stroke extends outside the path it follows: half its width, times the miter
/// limit for a miter join and times the square root of two for a square cap, whichever is larger.
pub(crate) fn stroke_reach(stroke: &PathStroke) -> f64 {
    let style = &stroke.style;
    let join = if style.join == Join::Miter {
        style.miter_limit
    } else {
        1.0
    };
    let cap = if style.start_cap == Cap::Square || style.end_cap == Cap::Square {
        core::f64::consts::SQRT_2
    } else {
        1.0
    };
    style.width * 0.5 * join.max(cap).max(1.0)
}
