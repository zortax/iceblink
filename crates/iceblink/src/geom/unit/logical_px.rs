//! The logical pixel.

use core::fmt::{self, Display};

use crate::geom::unit::{DevicePx, Unit, length_ops};

/// A length in logical pixels, the unit of the [`Logical`](crate::geom::Logical) space.
///
/// A logical pixel is independent of the output's pixel density. A
/// [`Scale<Logical, Device>`](crate::geom::Scale) converts it to [`DevicePx`].
///
/// ```
/// use iceblink::geom::LogicalPx;
///
/// let width = LogicalPx(12.0);
/// assert_eq!(width * 2.0, LogicalPx(24.0));
/// assert_eq!(width + LogicalPx(0.5), LogicalPx(12.5));
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct LogicalPx(pub f32);

impl LogicalPx {
    /// A length of zero.
    pub const ZERO: Self = Self(0.0);

    /// One logical pixel.
    pub const ONE: Self = Self(1.0);

    /// Returns the same number as a device pixel length, applying no scale.
    ///
    /// The result is a correct device length only where the scale factor is one or the value is
    /// a ratio.
    #[inline]
    pub const fn as_device_px_unscaled(self) -> DevicePx {
        DevicePx(self.0)
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

impl Unit for LogicalPx {
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

impl Display for LogicalPx {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}px", self.0)
    }
}

impl From<f32> for LogicalPx {
    #[inline]
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<LogicalPx> for f32 {
    #[inline]
    fn from(value: LogicalPx) -> Self {
        value.0
    }
}

length_ops!(LogicalPx, f32);

#[cfg(test)]
mod tests {
    use super::LogicalPx;

    #[test]
    fn arithmetic_stays_in_the_unit() {
        assert_eq!(LogicalPx(3.0) + LogicalPx(4.0), LogicalPx(7.0));
        assert_eq!(LogicalPx(3.0) - LogicalPx(4.0), LogicalPx(-1.0));
        assert_eq!(-LogicalPx(3.0), LogicalPx(-3.0));
        assert_eq!(LogicalPx(3.0) * 2.0, LogicalPx(6.0));
        assert_eq!(LogicalPx(3.0) / 2.0, LogicalPx(1.5));
    }

    #[test]
    fn dividing_two_lengths_gives_a_ratio() {
        assert_eq!(LogicalPx(6.0) / LogicalPx(2.0), 3.0);
    }

    #[test]
    fn summing_an_empty_iterator_gives_zero() {
        let total: LogicalPx = [].into_iter().sum();
        assert_eq!(total, LogicalPx::ZERO);
    }

    #[test]
    fn displays_with_its_suffix() {
        assert_eq!(LogicalPx(12.5).to_string(), "12.5px");
    }
}
