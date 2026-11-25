//! The scalar units lengths are measured in.
//!
//! - [`LogicalPx`] is a resolution-independent pixel.
//! - [`DevicePx`] is a physical pixel on the output surface.
//! - [`Scale`] is the ratio between two [spaces](crate::geom::space), such as an output's pixel
//!   density.
//!
//! Both units implement [`Unit`], which [`Point`](crate::geom::Point), [`Size`](crate::geom::Size)
//! and [`Rect`](crate::geom::Rect) are generic over. [`f32`] and [`i32`] implement it too, so a
//! rectangle of whole device pixels is a `Rect<i32, Device>`.
//!
//! Positions in two units do not combine, even within one space:
//!
//! ```compile_fail,E0277
//! # use iceblink::geom::{DevicePx, Logical, LogicalPx, Point};
//! let logical: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.0), LogicalPx(2.0));
//! let device: Point<DevicePx, Logical> = Point::new(DevicePx(3.0), DevicePx(4.0));
//! let _ = logical + device;
//! ```

pub mod device_px;
pub mod logical_px;
pub mod scale;

mod ops;

use core::fmt::Debug;
use core::ops::{Add, Neg, Sub};

use bytemuck::Pod;

pub use crate::geom::unit::device_px::DevicePx;
pub use crate::geom::unit::logical_px::LogicalPx;
pub use crate::geom::unit::scale::Scale;

pub(crate) use crate::geom::unit::ops::length_ops;

/// A scalar a length can be measured in.
///
/// A unit can be added, subtracted, negated and ordered, and it converts to and from [`f32`] so
/// that transforms and scale factors apply to it. Implemented for [`LogicalPx`], [`DevicePx`],
/// [`f32`] and [`i32`].
pub trait Unit:
    Copy
    + Debug
    + Default
    + PartialEq
    + PartialOrd
    + Pod
    + Send
    + Sync
    + Add<Output = Self>
    + Sub<Output = Self>
    + Neg<Output = Self>
    + 'static
{
    /// The additive identity.
    const ZERO: Self;

    /// The value one unit away from [`Unit::ZERO`].
    const ONE: Self;

    /// Converts from a plain [`f32`] count of this unit.
    ///
    /// Integer units round to nearest with ties away from zero, saturate at their bounds and map
    /// NaN to zero.
    fn from_f32(value: f32) -> Self;

    /// Converts to a plain [`f32`] count of this unit.
    fn to_f32(self) -> f32;

    /// Returns the smaller of two values.
    fn min(self, other: Self) -> Self;

    /// Returns the larger of two values.
    fn max(self, other: Self) -> Self;

    /// Returns the value clamped to `low ..= high`.
    ///
    /// # Panics
    ///
    /// Panics if `low > high`.
    #[inline]
    fn clamp(self, low: Self, high: Self) -> Self {
        assert!(low <= high, "clamp needs low <= high");
        self.max(low).min(high)
    }
}

impl Unit for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;

    #[inline]
    fn from_f32(value: f32) -> Self {
        value
    }

    #[inline]
    fn to_f32(self) -> f32 {
        self
    }

    #[inline]
    fn min(self, other: Self) -> Self {
        f32::min(self, other)
    }

    #[inline]
    fn max(self, other: Self) -> Self {
        f32::max(self, other)
    }
}

impl Unit for i32 {
    const ZERO: Self = 0;
    const ONE: Self = 1;

    #[inline]
    fn from_f32(value: f32) -> Self {
        round_saturating(value)
    }

    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }

    #[inline]
    fn min(self, other: Self) -> Self {
        Ord::min(self, other)
    }

    #[inline]
    fn max(self, other: Self) -> Self {
        Ord::max(self, other)
    }
}

/// Rounds to the nearest integer, ties away from zero, saturating at the [`i32`] bounds and
/// mapping NaN to zero.
pub(crate) fn round_saturating(value: f32) -> i32 {
    // `as` already saturates out-of-range and infinite floats.
    if value.is_nan() {
        0
    } else {
        value.round() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::{Unit, round_saturating};

    #[test]
    fn rounding_saturates_instead_of_wrapping() {
        assert_eq!(round_saturating(0.5), 1);
        assert_eq!(round_saturating(-0.5), -1);
        assert_eq!(round_saturating(f32::INFINITY), i32::MAX);
        assert_eq!(round_saturating(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(round_saturating(f32::NAN), 0);
        assert_eq!(round_saturating(1e30), i32::MAX);
    }

    #[test]
    fn clamp_orders_its_bounds() {
        assert_eq!(Unit::clamp(5.0_f32, 0.0, 3.0), 3.0);
        assert_eq!(Unit::clamp(-5_i32, 0, 3), 0);
    }
}
