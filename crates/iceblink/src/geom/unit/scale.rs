//! The ratio between two coordinate spaces.

use core::fmt::{self, Debug};
use core::marker::PhantomData;

use crate::geom::space::{Device, Logical, Space};

/// The ratio that converts lengths in space `Src` into lengths in space `Dst`.
///
/// An output's pixel density is a `Scale<Logical, Device>`. Both spaces are part of the type, so
/// a scale only applies in its own direction, and [`Scale::invert`] turns it around.
///
/// ```
/// use iceblink::geom::{Device, DevicePx, Logical, LogicalPx, Point, Scale};
///
/// let to_device: Scale<Logical, Device> = Scale::new(2.0);
/// let logical: Point<LogicalPx, Logical> = Point::new(LogicalPx(4.0), LogicalPx(8.0));
///
/// let device: Point<DevicePx, Device> = logical * to_device;
/// assert_eq!(device, Point::new(DevicePx(8.0), DevicePx(16.0)));
/// assert_eq!(device * to_device.invert(), logical);
/// ```
///
/// A scale does not apply in the opposite direction:
///
/// ```compile_fail,E0308
/// # use iceblink::geom::{Device, Logical, LogicalPx, Point, Scale};
/// let logical: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.0), LogicalPx(2.0));
/// let to_logical: Scale<Device, Logical> = Scale::new(0.5);
/// let _ = logical * to_logical;
/// ```
///
/// A scale converts without rounding. The functions in [`snap`](crate::geom::snap) take a scale and
/// round the result onto the device pixel grid.
#[repr(transparent)]
pub struct Scale<Src, Dst> {
    /// How many `Dst` units one `Src` unit is worth.
    pub(crate) factor: f32,
    pub(crate) spaces: PhantomData<(Src, Dst)>,
}

impl<Src, Dst> Scale<Src, Dst> {
    /// Creates a ratio of `factor` destination units per source unit.
    pub const fn new(factor: f32) -> Self {
        Self {
            factor,
            spaces: PhantomData,
        }
    }

    /// Returns the ratio as a plain number.
    pub const fn get(self) -> f32 {
        self.factor
    }

    /// Returns the ratio that undoes this one.
    ///
    /// ```
    /// use iceblink::geom::{Logical, Device, Scale};
    ///
    /// let to_device: Scale<Logical, Device> = Scale::new(4.0);
    /// assert_eq!(to_device.invert().get(), 0.25);
    /// ```
    pub fn invert(self) -> Scale<Dst, Src> {
        Scale::new(self.factor.recip())
    }

    /// Returns this ratio followed by `next`, which converts from `Src` straight to `Next`.
    pub fn then<Next>(self, next: Scale<Dst, Next>) -> Scale<Src, Next> {
        Scale::new(self.factor * next.factor)
    }
}

impl<S> Scale<S, S> {
    /// A ratio of one.
    pub const IDENTITY: Self = Self::new(1.0);
}

impl<Src, Dst> Clone for Scale<Src, Dst> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Src, Dst> Copy for Scale<Src, Dst> {}

impl<Src: Space, Dst: Space> Debug for Scale<Src, Dst> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Scale({} {} -> {})",
            self.factor,
            Src::NAME,
            Dst::NAME
        )
    }
}

impl<Src, Dst> PartialEq for Scale<Src, Dst> {
    fn eq(&self, other: &Self) -> bool {
        self.factor == other.factor
    }
}

/// Implements the conversions for one ordered pair of spaces and their units.
macro_rules! conversion {
    ($src:ty => $dst:ty, $from:ty => $to:ty) => {
        impl Scale<$src, $dst> {
            /// Converts a length into the destination space.
            pub fn apply_length(self, length: $from) -> $to {
                <$to>::from(<f32>::from(length) * self.factor)
            }

            /// Converts a position into the destination space.
            pub fn apply_point(
                self,
                point: $crate::geom::point::Point<$from, $src>,
            ) -> $crate::geom::point::Point<$to, $dst> {
                $crate::geom::point::Point::new(
                    self.apply_length(point.x),
                    self.apply_length(point.y),
                )
            }

            /// Converts an extent into the destination space.
            pub fn apply_size(
                self,
                size: $crate::geom::size::Size<$from, $src>,
            ) -> $crate::geom::size::Size<$to, $dst> {
                $crate::geom::size::Size::new(
                    self.apply_length(size.width),
                    self.apply_length(size.height),
                )
            }

            /// Converts a rectangle into the destination space.
            pub fn apply_rect(
                self,
                rect: $crate::geom::rect::Rect<$from, $src>,
            ) -> $crate::geom::rect::Rect<$to, $dst> {
                $crate::geom::rect::Rect::new(
                    self.apply_point(rect.origin),
                    self.apply_size(rect.size),
                )
            }

            /// Converts per-side lengths into the destination space.
            pub fn apply_edges(
                self,
                edges: $crate::geom::edges::Edges<$from>,
            ) -> $crate::geom::edges::Edges<$to> {
                edges.map(|length| self.apply_length(length))
            }
        }

        impl ::core::ops::Mul<Scale<$src, $dst>> for $crate::geom::point::Point<$from, $src> {
            type Output = $crate::geom::point::Point<$to, $dst>;

            fn mul(self, scale: Scale<$src, $dst>) -> Self::Output {
                scale.apply_point(self)
            }
        }

        impl ::core::ops::Mul<Scale<$src, $dst>> for $crate::geom::size::Size<$from, $src> {
            type Output = $crate::geom::size::Size<$to, $dst>;

            fn mul(self, scale: Scale<$src, $dst>) -> Self::Output {
                scale.apply_size(self)
            }
        }

        impl ::core::ops::Mul<Scale<$src, $dst>> for $crate::geom::rect::Rect<$from, $src> {
            type Output = $crate::geom::rect::Rect<$to, $dst>;

            fn mul(self, scale: Scale<$src, $dst>) -> Self::Output {
                scale.apply_rect(self)
            }
        }
    };
}

conversion!(Logical => Device, crate::geom::unit::LogicalPx => crate::geom::unit::DevicePx);
conversion!(Device => Logical, crate::geom::unit::DevicePx => crate::geom::unit::LogicalPx);

#[cfg(test)]
mod tests {
    use super::Scale;
    use crate::geom::point::Point;
    use crate::geom::rect::Rect;
    use crate::geom::size::Size;
    use crate::geom::space::{Device, Logical};
    use crate::geom::unit::{DevicePx, LogicalPx};

    #[test]
    fn a_scale_and_its_inverse_cancel() {
        let scale: Scale<Logical, Device> = Scale::new(2.0);
        let rect: Rect<LogicalPx, Logical> = Rect::new(
            Point::new(LogicalPx(1.0), LogicalPx(2.0)),
            Size::new(LogicalPx(3.0), LogicalPx(4.0)),
        );
        assert_eq!((rect * scale) * scale.invert(), rect);
    }

    #[test]
    fn composing_scales_multiplies_them() {
        let there: Scale<Logical, Device> = Scale::new(3.0);
        let back: Scale<Device, Logical> = Scale::new(0.5);
        assert_eq!(there.then(back), Scale::<Logical, Logical>::new(1.5));
    }

    #[test]
    fn the_identity_leaves_lengths_alone() {
        assert_eq!(Scale::<Logical, Logical>::IDENTITY.get(), 1.0);
    }

    #[test]
    fn scaling_reaches_the_other_unit_and_space() {
        let scale: Scale<Logical, Device> = Scale::new(1.5);
        assert_eq!(scale.apply_length(LogicalPx(10.0)), DevicePx(15.0));
        let size: Size<LogicalPx, Logical> = Size::new(LogicalPx(2.0), LogicalPx(4.0));
        assert_eq!(size * scale, Size::new(DevicePx(3.0), DevicePx(6.0)));
    }

    #[test]
    fn the_debug_output_names_both_spaces() {
        let scale: Scale<Logical, Device> = Scale::new(2.0);
        assert_eq!(format!("{scale:?}"), "Scale(2 logical -> device)");
    }
}
