// DERIVED-FROM: the GPUI project, crates/gpui/src/window.rs (Apache-2.0)
// The two rectangle snapping rules here are adapted from that work, licensed under the Apache
// License, Version 2.0, and modified to work over this crate's coordinate spaces and units. One
// rounds every edge to the nearest device pixel. The other floors the near edges and ceils the far
// ones, so the result covers the input.

//! Rectangle snapping.

use crate::geom::point::Point;
use crate::geom::rect::Rect;
use crate::geom::snap::{ceil_length, device_length, floor_length, snap_length};
use crate::geom::space::{Device, Space};
use crate::geom::unit::{DevicePx, Scale, Unit};

/// Scales a rectangle into device space and rounds every edge to the nearest device pixel.
///
/// The far edges never move past the near ones, so the result can collapse to zero extent but is
/// never inverted.
///
/// ```
/// use iceblink::geom::{
///     Device, DevicePx, Logical, LogicalPx, Point, Rect, Scale, Size, snap_bounds,
/// };
///
/// let scale: Scale<Logical, Device> = Scale::new(1.5);
/// let rect: Rect<LogicalPx, Logical> = Rect::new(
///     Point::new(LogicalPx(1.0), LogicalPx(1.0)),
///     Size::new(LogicalPx(10.0), LogicalPx(10.0)),
/// );
/// let snapped = snap_bounds(rect, scale);
/// // 1.5 and 16.5 device pixels are exact halves, which round toward zero.
/// assert_eq!(snapped.origin, Point::new(DevicePx(1.0), DevicePx(1.0)));
/// assert_eq!(snapped.far_corner(), Point::new(DevicePx(16.0), DevicePx(16.0)));
/// ```
pub fn snap_bounds<T: Unit, S: Space>(
    rect: Rect<T, S>,
    scale: Scale<S, Device>,
) -> Rect<DevicePx, Device> {
    snap_device_bounds(to_device(rect, scale))
}

/// Rounds every edge of a device rectangle to the nearest device pixel.
///
/// This is the rule of [`snap_bounds`] without the scale. Applying it twice changes nothing.
pub fn snap_device_bounds(rect: Rect<DevicePx, Device>) -> Rect<DevicePx, Device> {
    let near = rect.origin.map(snap_length);
    let far = rect.far_corner().map(snap_length).max(near);
    Rect::new(near, far.offset_from(near))
}

/// Scales a rectangle into device space and grows it to the smallest whole-pixel rectangle that
/// contains it.
///
/// ```
/// use iceblink::geom::{
///     Device, DevicePx, Logical, LogicalPx, Point, Rect, Scale, Size, cover_bounds,
/// };
///
/// let scale: Scale<Logical, Device> = Scale::new(1.0);
/// let rect: Rect<LogicalPx, Logical> = Rect::new(
///     Point::new(LogicalPx(0.25), LogicalPx(0.25)),
///     Size::new(LogicalPx(1.5), LogicalPx(1.5)),
/// );
/// let covered = cover_bounds(rect, scale);
/// assert_eq!(covered.origin, Point::new(DevicePx(0.0), DevicePx(0.0)));
/// assert_eq!(covered.far_corner(), Point::new(DevicePx(2.0), DevicePx(2.0)));
/// ```
pub fn cover_bounds<T: Unit, S: Space>(
    rect: Rect<T, S>,
    scale: Scale<S, Device>,
) -> Rect<DevicePx, Device> {
    cover_device_bounds(to_device(rect, scale))
}

/// Grows a device rectangle to the smallest whole-pixel rectangle that contains it.
///
/// This is the rule of [`cover_bounds`] without the scale. Applying it twice changes nothing.
pub fn cover_device_bounds(rect: Rect<DevicePx, Device>) -> Rect<DevicePx, Device> {
    let near: Point<DevicePx, Device> = rect.origin.map(floor_length);
    let far = rect.far_corner().map(ceil_length).max(near);
    Rect::new(near, far.offset_from(near))
}

/// Scales a rectangle's origin and extent into device space without rounding.
fn to_device<T: Unit, S: Space>(
    rect: Rect<T, S>,
    scale: Scale<S, Device>,
) -> Rect<DevicePx, Device> {
    let length = |value: T| device_length(value, scale.get());
    Rect::new(
        rect.origin.map(length).cast_space(),
        rect.size.map(length).cast_space(),
    )
}
