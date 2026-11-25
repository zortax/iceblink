//! A two-dimensional extent in a coordinate space.

use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Sub, SubAssign};

use crate::geom::space::Space;
use crate::geom::space::derive::space_derives;
use crate::geom::unit::Unit;

/// A width and a height measured in unit `T`, in coordinate space `S`.
///
/// A size is also the offset between two positions, as returned by
/// [`Point::offset_from`](crate::geom::Point::offset_from).
///
/// ```
/// use iceblink::geom::{Logical, LogicalPx, Size};
///
/// let size: Size<LogicalPx, Logical> = Size::new(LogicalPx(120.0), LogicalPx(40.0));
/// assert_eq!(size.area(), 4_800.0);
/// assert!(!size.is_empty());
/// ```
#[repr(C)]
pub struct Size<T, S> {
    /// The horizontal extent.
    pub width: T,
    /// The vertical extent.
    pub height: T,
    pub(crate) space: PhantomData<S>,
}

space_derives!(Size { width, height });

impl<T, S> Size<T, S> {
    /// Creates an extent with the given width and height.
    #[inline]
    pub const fn new(width: T, height: T) -> Self {
        Self {
            width,
            height,
            space: PhantomData,
        }
    }

    /// Applies `function` to both extents, possibly changing the unit but keeping the space.
    #[inline]
    pub fn map<U>(self, mut function: impl FnMut(T) -> U) -> Size<U, S> {
        Size::new(function(self.width), function(self.height))
    }

    /// Returns the same numbers as an extent in space `D`.
    ///
    /// The result is correct only where the two spaces coincide. A [`Scale`](crate::geom::Scale)
    /// converts between spaces that differ.
    #[inline]
    pub fn cast_space<D>(self) -> Size<T, D> {
        Size::new(self.width, self.height)
    }
}

impl<T: Unit, S: Space> Size<T, S> {
    /// An extent of zero in both directions.
    pub const ZERO: Self = Self {
        width: T::ZERO,
        height: T::ZERO,
        space: PhantomData,
    };

    /// Creates an extent of `side` in both directions.
    #[inline]
    pub fn square(side: T) -> Self {
        Self::new(side, side)
    }

    /// Reports whether either extent is zero or negative.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.width <= T::ZERO || self.height <= T::ZERO
    }

    /// Returns the product of the two extents.
    #[inline]
    pub fn area(self) -> f32 {
        self.width.to_f32() * self.height.to_f32()
    }

    /// Returns the componentwise minimum of two extents.
    #[inline]
    pub fn min(self, other: Self) -> Self {
        Self::new(self.width.min(other.width), self.height.min(other.height))
    }

    /// Returns the componentwise maximum of two extents.
    #[inline]
    pub fn max(self, other: Self) -> Self {
        Self::new(self.width.max(other.width), self.height.max(other.height))
    }

    /// Clamps both extents to at least zero.
    #[inline]
    pub fn non_negative(self) -> Self {
        Self::new(self.width.max(T::ZERO), self.height.max(T::ZERO))
    }

    /// Converts both extents to unit `U` through [`f32`].
    #[inline]
    pub fn to_unit<U: Unit>(self) -> Size<U, S> {
        Size::new(
            U::from_f32(self.width.to_f32()),
            U::from_f32(self.height.to_f32()),
        )
    }
}

impl<T: Unit, S: Space> Add for Size<T, S> {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self::new(self.width + other.width, self.height + other.height)
    }
}

impl<T: Unit, S: Space> AddAssign for Size<T, S> {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<T: Unit, S: Space> Sub for Size<T, S> {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self::new(self.width - other.width, self.height - other.height)
    }
}

impl<T: Unit, S: Space> SubAssign for Size<T, S> {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}
