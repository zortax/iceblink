// DERIVED-FROM: the GPUI project, crates/gpui/src/window.rs and crates/gpui/src/util.rs
// (Apache-2.0) The stroke snapping rule is adapted from that work, licensed under the Apache
// License, Version 2.0, and modified to work over this crate's spaces and units. It rounds to the
// nearest device pixel, and never lets a non-zero stroke round away to nothing.

//! Stroke width snapping.

use crate::geom::edges::Edges;
use crate::geom::snap::{device_length, snap_length};
use crate::geom::space::{Device, Space};
use crate::geom::unit::{DevicePx, Scale, Unit};

/// Scales a stroke width into device space and rounds it to the nearest device pixel, never
/// rounding a positive width to zero.
///
/// A width of zero or less becomes zero. A positive width becomes at least one device pixel.
///
/// ```
/// use iceblink::geom::{Device, DevicePx, Logical, LogicalPx, Scale, snap_stroke};
///
/// let scale: Scale<Logical, Device> = Scale::new(1.0);
/// assert_eq!(snap_stroke(LogicalPx(0.0), scale), DevicePx(0.0));
/// assert_eq!(snap_stroke(LogicalPx(0.4), scale), DevicePx(1.0));
/// assert_eq!(snap_stroke(LogicalPx(2.6), scale), DevicePx(3.0));
/// ```
pub fn snap_stroke<T: Unit, S: Space>(width: T, scale: Scale<S, Device>) -> DevicePx {
    snap_device_stroke(device_length(width, scale.get()))
}

/// Rounds a device stroke width to the nearest device pixel, never rounding a positive width to
/// zero.
///
/// This is the rule of [`snap_stroke`] without the scale. Applying it twice changes nothing.
pub fn snap_device_stroke(width: DevicePx) -> DevicePx {
    if width.0 <= 0.0 {
        return DevicePx(0.0);
    }

    Unit::max(snap_length(width), DevicePx(1.0))
}

/// Snaps all four widths of `edges` by the rule of [`snap_stroke`].
///
/// ```
/// use iceblink::geom::{Device, DevicePx, Edges, Logical, LogicalPx, Scale, snap_edges};
///
/// let scale: Scale<Logical, Device> = Scale::new(2.0);
/// let border = Edges::new(LogicalPx(1.0), LogicalPx(0.0), LogicalPx(0.2), LogicalPx(1.0));
/// assert_eq!(
///     snap_edges(border, scale),
///     Edges::new(DevicePx(2.0), DevicePx(0.0), DevicePx(1.0), DevicePx(2.0)),
/// );
/// ```
pub fn snap_edges<T: Unit, S: Space>(edges: Edges<T>, scale: Scale<S, Device>) -> Edges<DevicePx> {
    edges.map(|width| snap_stroke(width, scale))
}
