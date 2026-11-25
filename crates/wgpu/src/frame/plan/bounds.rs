//! Device rectangles of the frame's float bounds.

use iceblink::geom::{Device, Point, Rect};

/// Returns the whole device pixels a rectangle `[x, y, width, height]` touches.
pub(super) fn rounded_out(rect: [f32; 4]) -> Rect<i32, Device> {
    let [x, y, width, height] = rect;
    Rect::from_corners(
        Point::new(x.floor() as i32, y.floor() as i32),
        Point::new((x + width).ceil() as i32, (y + height).ceil() as i32),
    )
}

/// Returns `bounds` cut to `rect` and `used`, or an empty rectangle if they do not meet.
pub(super) fn clamped(
    bounds: Rect<i32, Device>,
    rect: Rect<i32, Device>,
    used: Rect<i32, Device>,
) -> Rect<i32, Device> {
    bounds
        .intersection(rect)
        .and_then(|clipped| clipped.intersection(used))
        .unwrap_or(Rect::ZERO)
}
