//! The device pixel.

use core::fmt::{self, Display};

use crate::geom::unit::{LogicalPx, Unit, length_ops};

/// A length in physical pixels on the output surface, the unit of the
/// [`Device`](crate::geom::Device) space.
///
/// An edge at a whole number of device pixels lies on a pixel boundary. A
/// [`Scale<Logical, Device>`](crate::geom::Scale) converts [`LogicalPx`] to this unit, and the
/// functions in [`snap`](crate::geom::snap) round it onto the grid.
///
/// ```
/// use iceblink::geom::{Device, DevicePx, Logical, LogicalPx, Scale};
///
/// let scale: Scale<Logical, Device> = Scale::new(1.5);
/// assert_eq!(scale.apply_length(LogicalPx(10.0)), DevicePx(15.0));
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct DevicePx(pub f32);

impl DevicePx {
    /// A length of zero.
    pub const ZERO: Self = Self(0.0);

    /// One device pixel.
    pub const ONE: Self = Self(1.0);

    /// Returns the same number as a logical pixel length, applying no scale.
    ///
    /// The result is a correct logical length only where the scale factor is one or the value is
    /// a ratio.
    #[inline]
    pub const fn as_logical_px_unscaled(self) -> LogicalPx {
        LogicalPx(self.0)
    }

    /// Reports whether the length is a whole number of device pixels.
    ///
    /// ```
    /// use iceblink::geom::DevicePx;
    ///
    /// assert!(DevicePx(3.0).is_grid_aligned());
    /// assert!(!DevicePx(3.5).is_grid_aligned());
    /// ```
    #[inline]
    pub fn is_grid_aligned(self) -> bool {
        self.0.fract() == 0.0
    }

    /// Returns the absolute value.
    #[inline]
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Reports whether the length is neither infinite nor NaN.
    #[inline]
    pub fn is_finite(self) -> bool {
        self.0.is_finite()
    }

    #[inline]
    fn from_scaled(value: f32, factor: f32) -> Self {
        Self(value * factor)
    }

    #[inline]
    fn from_divided(value: f32, divisor: f32) -> Self {
        Self(value / divisor)
    }
}

impl Unit for DevicePx {
    const ZERO: Self = Self::ZERO;
    const ONE: Self = Self::ONE;

    #[inline]
    fn from_f32(value: f32) -> Self {
        Self(value)
    }

    #[inline]
    fn to_f32(self) -> f32 {
        self.0
    }

    #[inline]
    fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    #[inline]
    fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }
}

impl Display for DevicePx {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}dp", self.0)
    }
}

impl From<f32> for DevicePx {
    #[inline]
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<DevicePx> for f32 {
    #[inline]
    fn from(value: DevicePx) -> Self {
        value.0
    }
}

length_ops!(DevicePx, f32);

#[cfg(test)]
mod tests {
    use super::DevicePx;

    #[test]
    fn arithmetic_stays_in_the_unit() {
        assert_eq!(DevicePx(3.0) + DevicePx(4.0), DevicePx(7.0));
        assert_eq!(DevicePx(3.0) * 2.0, DevicePx(6.0));
        assert_eq!(DevicePx(6.0) / DevicePx(2.0), 3.0);
    }

    #[test]
    fn grid_alignment_is_about_the_fractional_part() {
        assert!(DevicePx(-4.0).is_grid_aligned());
        assert!(!DevicePx(-4.25).is_grid_aligned());
    }

    #[test]
    fn displays_with_its_own_suffix() {
        assert_eq!(DevicePx(4.0).to_string(), "4dp");
    }
}
