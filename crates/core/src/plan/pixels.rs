//! Conversions from fractional device rectangles to whole pixels.

use iceblink::DeviceRect;
use iceblink::geom::{Device, DevicePx, Point, Rect, Size};

/// Returns the smallest whole-pixel rectangle containing `rect`, the rounding of a device ink.
pub fn covering(rect: Rect<DevicePx, Device>) -> DeviceRect {
    let left = rect.origin.x.0.floor() as i32;
    let top = rect.origin.y.0.floor() as i32;
    let right = (rect.origin.x.0 + rect.size.width.0).ceil() as i32;
    let bottom = (rect.origin.y.0 + rect.size.height.0).ceil() as i32;
    DeviceRect::new(
        Point::new(left, top),
        Size::new((right - left).max(0), (bottom - top).max(0)),
    )
}

/// Returns the whole pixels `rect` covers completely, inset by one more pixel on every side.
///
/// An opaque quad with device bounds `rect` replaces every pixel of the result.
// The extra pixel is the one the edge antialiasing softens.
pub fn inner_pixels(rect: Rect<DevicePx, Device>) -> DeviceRect {
    let left = (rect.origin.x.0.ceil() as i32).saturating_add(1);
    let top = (rect.origin.y.0.ceil() as i32).saturating_add(1);
    let right = ((rect.origin.x.0 + rect.size.width.0).floor() as i32).saturating_sub(1);
    let bottom = ((rect.origin.y.0 + rect.size.height.0).floor() as i32).saturating_sub(1);
    DeviceRect::from_corners(
        Point::new(left, top),
        Point::new(right.max(left), bottom.max(top)),
    )
}

/// Returns the rectangle of an output of `size`.
pub(crate) fn output(size: [u32; 2]) -> DeviceRect {
    DeviceRect::new(
        Point::new(0, 0),
        Size::new(
            size[0].min(i32::MAX as u32) as i32,
            size[1].min(i32::MAX as u32) as i32,
        ),
    )
}
