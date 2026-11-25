//! Per-side lengths around a box.

use core::ops::{Add, AddAssign, Sub, SubAssign};

use crate::geom::size::Size;
use crate::geom::space::Space;
use crate::geom::unit::Unit;

/// Four lengths, one per side of a rectangle, such as a border width or an inset.
///
/// Edges carry no space. They take the space of the rectangle they are applied to through
/// [`Rect::inset`](crate::geom::Rect::inset) or [`Rect::outset`](crate::geom::Rect::outset).
///
/// ```
/// use iceblink::geom::{Edges, Logical, LogicalPx, Point, Rect, Size};
///
/// let border = Edges::uniform(LogicalPx(2.0));
/// let outer: Rect<LogicalPx, Logical> = Rect::new(
///     Point::new(LogicalPx(0.0), LogicalPx(0.0)),
///     Size::new(LogicalPx(100.0), LogicalPx(50.0)),
/// );
/// let inner = outer.inset(border);
/// assert_eq!(inner.size, Size::new(LogicalPx(96.0), LogicalPx(46.0)));
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Edges<T> {
    /// The length along the top side.
    pub top: T,
    /// The length along the right side.
    pub right: T,
    /// The length along the bottom side.
    pub bottom: T,
    /// The length along the left side.
    pub left: T,
}

impl<T> Edges<T> {
    /// Creates edges from lengths in the order top, right, bottom, left.
    #[inline]
    pub const fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Applies `function` to every side, possibly changing the unit.
    #[inline]
    pub fn map<U>(self, mut function: impl FnMut(T) -> U) -> Edges<U> {
        Edges {
            top: function(self.top),
            right: function(self.right),
            bottom: function(self.bottom),
            left: function(self.left),
        }
    }

    /// Returns the four lengths in the order top, right, bottom, left.
    #[inline]
    pub fn into_array(self) -> [T; 4] {
        [self.top, self.right, self.bottom, self.left]
    }
}

impl<T: Copy> Edges<T> {
    /// Creates edges with `length` on all four sides.
    #[inline]
    pub const fn uniform(length: T) -> Self {
        Self {
            top: length,
            right: length,
            bottom: length,
            left: length,
        }
    }

    /// Creates edges with `horizontal` on the left and right and `vertical` on the top and bottom.
    #[inline]
    pub const fn axes(horizontal: T, vertical: T) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

impl<T: Unit> Edges<T> {
    /// No length on any side.
    pub const ZERO: Self = Self {
        top: T::ZERO,
        right: T::ZERO,
        bottom: T::ZERO,
        left: T::ZERO,
    };

    /// Returns the left plus the right length.
    #[inline]
    pub fn horizontal(self) -> T {
        self.left + self.right
    }

    /// Returns the top plus the bottom length.
    #[inline]
    pub fn vertical(self) -> T {
        self.top + self.bottom
    }

    /// Returns the horizontal and vertical totals as a size in space `S`.
    ///
    /// ```
    /// use iceblink::geom::{Logical, LogicalPx, Edges, Size};
    ///
    /// let padding = Edges::uniform(LogicalPx(4.0));
    /// assert_eq!(padding.total::<Logical>(), Size::new(LogicalPx(8.0), LogicalPx(8.0)));
    /// ```
    #[inline]
    pub fn total<S: Space>(self) -> Size<T, S> {
        Size::new(self.horizontal(), self.vertical())
    }

    /// Reports whether every side is zero.
    #[inline]
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    /// Clamps every side to at least zero.
    #[inline]
    pub fn non_negative(self) -> Self {
        self.map(|length| length.max(T::ZERO))
    }

    /// Converts every side to unit `U` through [`f32`].
    #[inline]
    pub fn to_unit<U: Unit>(self) -> Edges<U> {
        self.map(|length| U::from_f32(length.to_f32()))
    }
}

impl<T: Unit> Add for Edges<T> {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self::new(
            self.top + other.top,
            self.right + other.right,
            self.bottom + other.bottom,
            self.left + other.left,
        )
    }
}

impl<T: Unit> AddAssign for Edges<T> {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<T: Unit> Sub for Edges<T> {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self::new(
            self.top - other.top,
            self.right - other.right,
            self.bottom - other.bottom,
            self.left - other.left,
        )
    }
}

impl<T: Unit> SubAssign for Edges<T> {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}
