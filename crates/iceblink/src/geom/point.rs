//! A position in a coordinate space.

use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use crate::geom::size::Size;
use crate::geom::space::Space;
use crate::geom::space::derive::space_derives;
use crate::geom::unit::Unit;

/// A position measured in unit `T`, in coordinate space `S`.
///
/// The space is a zero-sized marker. Positions from two spaces do not combine:
///
/// ```compile_fail,E0277
/// # use iceblink::geom::{Device, Logical, LogicalPx, Point};
/// let logical: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.0), LogicalPx(2.0));
/// let device: Point<LogicalPx, Device> = Point::new(LogicalPx(3.0), LogicalPx(4.0));
/// let _ = logical + device;
/// ```
///
/// Within one space the arithmetic is ordinary:
///
/// ```
/// use iceblink::geom::{Logical, LogicalPx, Point, Size};
///
/// let origin: Point<LogicalPx, Logical> = Point::new(LogicalPx(10.0), LogicalPx(20.0));
/// let moved = origin + Size::new(LogicalPx(5.0), LogicalPx(0.0));
/// assert_eq!(moved, Point::new(LogicalPx(15.0), LogicalPx(20.0)));
/// ```
#[repr(C)]
pub struct Point<T, S> {
    /// The horizontal coordinate, growing to the right.
    pub x: T,
    /// The vertical coordinate, growing downward.
    pub y: T,
    pub(crate) space: PhantomData<S>,
}

space_derives!(Point { x, y });

impl<T, S> Point<T, S> {
    /// Creates a position at the given coordinates.
    #[inline]
    pub const fn new(x: T, y: T) -> Self {
        Self {
            x,
            y,
            space: PhantomData,
        }
    }

    /// Applies `function` to both coordinates, possibly changing the unit but keeping the space.
    ///
    /// ```
    /// use iceblink::geom::{Logical, LogicalPx, Point};
    ///
    /// let point: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.5), LogicalPx(-2.5));
    /// assert_eq!(point.map(|value| value.abs()), Point::new(LogicalPx(1.5), LogicalPx(2.5)));
    /// ```
    #[inline]
    pub fn map<U>(self, mut function: impl FnMut(T) -> U) -> Point<U, S> {
        Point::new(function(self.x), function(self.y))
    }

    /// Returns the same numbers as a position in space `D`.
    ///
    /// The result is correct only where the two spaces coincide. A [`Scale`](crate::geom::Scale)
    /// converts between spaces that differ.
    #[inline]
    pub fn cast_space<D>(self) -> Point<T, D> {
        Point::new(self.x, self.y)
    }
}

impl<T: Unit, S: Space> Point<T, S> {
    /// The origin of the space.
    pub const ORIGIN: Self = Self {
        x: T::ZERO,
        y: T::ZERO,
        space: PhantomData,
    };

    /// Returns the offset from `other` to `self`.
    ///
    /// ```
    /// use iceblink::geom::{Logical, LogicalPx, Point, Size};
    ///
    /// let a: Point<LogicalPx, Logical> = Point::new(LogicalPx(10.0), LogicalPx(10.0));
    /// let b = Point::new(LogicalPx(4.0), LogicalPx(3.0));
    /// assert_eq!(a.offset_from(b), Size::new(LogicalPx(6.0), LogicalPx(7.0)));
    /// ```
    #[inline]
    pub fn offset_from(self, other: Self) -> Size<T, S> {
        Size::new(self.x - other.x, self.y - other.y)
    }

    /// Returns the componentwise minimum of two positions.
    #[inline]
    pub fn min(self, other: Self) -> Self {
        Self::new(self.x.min(other.x), self.y.min(other.y))
    }

    /// Returns the componentwise maximum of two positions.
    #[inline]
    pub fn max(self, other: Self) -> Self {
        Self::new(self.x.max(other.x), self.y.max(other.y))
    }

    /// Converts both coordinates to unit `U` through [`f32`].
    ///
    /// ```
    /// use iceblink::geom::{Logical, LogicalPx, Point};
    ///
    /// let point: Point<LogicalPx, Logical> = Point::new(LogicalPx(2.0), LogicalPx(3.0));
    /// assert_eq!(point.to_unit::<f32>(), Point::new(2.0, 3.0));
    /// ```
    #[inline]
    pub fn to_unit<U: Unit>(self) -> Point<U, S> {
        Point::new(U::from_f32(self.x.to_f32()), U::from_f32(self.y.to_f32()))
    }
}

impl<T: Unit, S: Space> Add<Self> for Point<T, S> {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl<T: Unit, S: Space> AddAssign<Self> for Point<T, S> {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<T: Unit, S: Space> Sub<Self> for Point<T, S> {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl<T: Unit, S: Space> SubAssign<Self> for Point<T, S> {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl<T: Unit, S: Space> Add<Size<T, S>> for Point<T, S> {
    type Output = Self;

    #[inline]
    fn add(self, offset: Size<T, S>) -> Self {
        Self::new(self.x + offset.width, self.y + offset.height)
    }
}

impl<T: Unit, S: Space> Sub<Size<T, S>> for Point<T, S> {
    type Output = Self;

    #[inline]
    fn sub(self, offset: Size<T, S>) -> Self {
        Self::new(self.x - offset.width, self.y - offset.height)
    }
}

impl<T: Unit, S: Space> Neg for Point<T, S> {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}
