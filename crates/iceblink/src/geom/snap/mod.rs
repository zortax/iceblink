// DERIVED-FROM: the GPUI project, crates/gpui/src/window.rs and crates/gpui/src/util.rs
// (Apache-2.0) The device-pixel snapping policy in this module is adapted from that work, licensed
// under the Apache License, Version 2.0, and modified to work over this crate's coordinate spaces
// and units. The policy rounds with ties toward zero, floors near edges and ceils far ones to cover
// the input, and never rounds a non-zero stroke away.

//! The device-pixel snapping rules.
//!
//! Each rule scales geometry into the [`Device`](crate::geom::Device) space and rounds it onto the
//! pixel grid:
//!
//! - [`snap_bounds`] rounds every edge to the nearest device pixel, for geometry that is drawn.
//! - [`cover_bounds`] rounds the near edges down and the far edges up, so the result contains the
//!   input. It is for geometry that bounds other geometry, such as a clip or a damage rectangle.
//! - [`snap_stroke`] rounds a stroke width to the nearest device pixel, but never rounds a
//!   positive width to zero.
//!
//! Exact halves round toward zero. Each rule has a form for geometry that is already in device
//! space, such as [`snap_device_bounds`].
//!
//! ```
//! use iceblink::geom::{
//!     Device, DevicePx, Logical, LogicalPx, Point, Rect, Scale, Size, cover_bounds, snap_bounds,
//! };
//!
//! let scale: Scale<Logical, Device> = Scale::new(2.0);
//! let rect: Rect<LogicalPx, Logical> = Rect::new(
//!     Point::new(LogicalPx(10.1), LogicalPx(10.1)),
//!     Size::new(LogicalPx(20.0), LogicalPx(20.0)),
//! );
//!
//! assert_eq!(snap_bounds(rect, scale).origin, Point::new(DevicePx(20.0), DevicePx(20.0)));
//! assert_eq!(cover_bounds(rect, scale).origin, Point::new(DevicePx(20.0), DevicePx(20.0)));
//! assert_eq!(cover_bounds(rect, scale).far_corner(), Point::new(DevicePx(61.0), DevicePx(61.0)));
//! ```

pub mod bounds;
pub mod stroke;

use crate::geom::unit::{DevicePx, Unit};

pub use crate::geom::snap::bounds::{
    cover_bounds, cover_device_bounds, snap_bounds, snap_device_bounds,
};
pub use crate::geom::snap::stroke::{snap_device_stroke, snap_edges, snap_stroke};

/// Rounds to the nearest integer, with exact halves going toward zero.
///
/// A whole number is returned unchanged at any magnitude, and a non-finite value is returned as
/// it is.
///
/// ```
/// use iceblink::geom::snap::round_half_toward_zero;
///
/// assert_eq!(round_half_toward_zero(2.5), 2.0);
/// assert_eq!(round_half_toward_zero(-2.5), -2.0);
/// assert_eq!(round_half_toward_zero(2.6), 3.0);
/// ```
pub fn round_half_toward_zero(value: f32) -> f32 {
    // Ties toward zero keep a shape centred on the origin the same size as its mirror image.
    // The whole part is split off first because past 2^23 an `f32` has no fraction left, and
    // comparing `value.abs() - 0.5` against the grid would move a whole coordinate by a pixel.
    let whole = value.trunc();
    if !whole.is_finite() {
        return value;
    }

    let fraction = value - whole;
    if fraction.abs() > 0.5 {
        whole + fraction.signum()
    } else {
        whole
    }
}

/// Rounds a device length to the nearest whole pixel, with exact halves going toward zero.
pub fn snap_length(length: DevicePx) -> DevicePx {
    DevicePx(round_half_toward_zero(length.0))
}

/// Rounds a device length down to a whole pixel.
pub fn floor_length(length: DevicePx) -> DevicePx {
    DevicePx(length.0.floor())
}

/// Rounds a device length up to a whole pixel.
pub fn ceil_length(length: DevicePx) -> DevicePx {
    DevicePx(length.0.ceil())
}

/// Multiplies a length by a scale factor into device pixels.
fn device_length<T: Unit>(length: T, factor: f32) -> DevicePx {
    DevicePx(length.to_f32() * factor)
}
