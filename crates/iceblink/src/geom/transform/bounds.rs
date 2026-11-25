//! The axis-aligned box a transformed rectangle occupies.

use crate::geom::point::Point;
use crate::geom::rect::Rect;
use crate::geom::size::Size;
use crate::geom::space::Device;
use crate::geom::transform::Matrix4;
use crate::geom::unit::DevicePx;

/// Returns the smallest axis-aligned rectangle containing the corners of `rect` projected through
/// `matrix`.
///
/// A corner whose `w` is zero or negative after the transform lies behind the viewer and is left
/// out. If every corner is left out, the result is [`Rect::ZERO`].
///
/// ```
/// use iceblink::geom::{Device, DevicePx, Matrix4, Point, Rect, Size, transformed_bounds};
///
/// let rect: Rect<DevicePx, Device> = Rect::new(
///     Point::new(DevicePx(0.0), DevicePx(0.0)),
///     Size::new(DevicePx(10.0), DevicePx(4.0)),
/// );
/// let moved = transformed_bounds(&Matrix4::translation(3.0, 5.0, 0.0), rect);
/// assert_eq!(moved.origin.x, DevicePx(3.0));
/// assert_eq!(moved.size.width, DevicePx(10.0));
/// ```
pub fn transformed_bounds(
    matrix: &Matrix4,
    rect: Rect<DevicePx, Device>,
) -> Rect<DevicePx, Device> {
    if let Some((x, y)) = matrix.plane_translation() {
        return translated_bounds(x, y, rect);
    }

    let corners = [
        (rect.left().0, rect.top().0),
        (rect.right().0, rect.top().0),
        (rect.right().0, rect.bottom().0),
        (rect.left().0, rect.bottom().0),
    ];

    let mut min = (f32::INFINITY, f32::INFINITY);
    let mut max = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut seen = 0;
    for (x, y) in corners {
        let projected = matrix.transform_vector4([x, y, 0.0, 1.0]);
        if projected[3] <= 0.0 {
            continue;
        }
        let inverse_w = projected[3].recip();
        let (px, py) = (projected[0] * inverse_w, projected[1] * inverse_w);
        min = (min.0.min(px), min.1.min(py));
        max = (max.0.max(px), max.1.max(py));
        seen += 1;
    }

    if seen == 0 {
        return Rect::ZERO;
    }

    Rect::from_corners(
        Point::new(DevicePx(min.0), DevicePx(min.1)),
        Point::new(DevicePx(max.0), DevicePx(max.1)),
    )
}

/// Returns the axis-aligned rectangle containing `rect` after a translation by `x` and `y`.
///
/// The result has the same bits as [`transformed_bounds`] through the matrix of that translation.
///
/// ```
/// use iceblink::geom::{
///     Device, DevicePx, Matrix4, Point, Rect, Size, transformed_bounds, translated_bounds,
/// };
///
/// let rect: Rect<DevicePx, Device> = Rect::new(
///     Point::new(DevicePx(0.5), DevicePx(2.0)),
///     Size::new(DevicePx(10.0), DevicePx(4.0)),
/// );
/// let matrix = Matrix4::translation(3.0, -5.0, 0.0);
/// assert_eq!(translated_bounds(3.0, -5.0, rect), transformed_bounds(&matrix, rect));
/// ```
#[inline]
pub fn translated_bounds(x: f32, y: f32, rect: Rect<DevicePx, Device>) -> Rect<DevicePx, Device> {
    // These are the sums the corner path of `transformed_bounds` makes, so both give the same bits.
    let (left, right) = (rect.left().0 + x, rect.right().0 + x);
    let (top, bottom) = (rect.top().0 + y, rect.bottom().0 + y);

    // Edges in order are their own minimum and maximum, so the comparisons below give this.
    if left < right && top < bottom {
        return Rect::new(
            Point::new(DevicePx(left), DevicePx(top)),
            Size::new(DevicePx(right - left), DevicePx(bottom - top)),
        );
    }

    Rect::from_corners(
        Point::new(DevicePx(left.min(right)), DevicePx(top.min(bottom))),
        Point::new(DevicePx(left.max(right)), DevicePx(top.max(bottom))),
    )
}
