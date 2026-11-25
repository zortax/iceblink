//! An axis-aligned rectangle in a coordinate space.

use crate::geom::edges::Edges;
use crate::geom::point::Point;
use crate::geom::size::Size;
use crate::geom::space::Space;
use crate::geom::space::derive::space_derives;
use crate::geom::unit::Unit;

/// An axis-aligned rectangle measured in unit `T`, in coordinate space `S`.
///
/// The origin is the top-left corner and the far edges lie at `origin + size`. A rectangle with a
/// zero or negative extent on either axis is empty and contains no points.
///
/// ```
/// use iceblink::geom::{Logical, LogicalPx, Point, Rect, Size};
///
/// let outer: Rect<LogicalPx, Logical> = Rect::new(
///     Point::new(LogicalPx(0.0), LogicalPx(0.0)),
///     Size::new(LogicalPx(100.0), LogicalPx(50.0)),
/// );
/// let inner = Rect::new(
///     Point::new(LogicalPx(10.0), LogicalPx(10.0)),
///     Size::new(LogicalPx(20.0), LogicalPx(20.0)),
/// );
/// assert!(outer.contains_rect(inner));
/// assert_eq!(outer.intersection(inner), Some(inner));
/// ```
#[repr(C)]
pub struct Rect<T, S> {
    /// The top-left corner.
    pub origin: Point<T, S>,
    /// The extent, growing to the right and downward from the origin.
    pub size: Size<T, S>,
}

space_derives!(Rect { origin, size } tagged by its fields);

impl<T, S> Rect<T, S> {
    /// Creates a rectangle at `origin` with extent `size`.
    #[inline]
    pub const fn new(origin: Point<T, S>, size: Size<T, S>) -> Self {
        Self { origin, size }
    }

    /// Returns the same numbers as a rectangle in space `D`.
    ///
    /// The result is correct only where the two spaces coincide. A [`Scale`](crate::geom::Scale)
    /// converts between spaces that differ.
    #[inline]
    pub fn cast_space<D>(self) -> Rect<T, D> {
        Rect::new(self.origin.cast_space(), self.size.cast_space())
    }
}

impl<T: Unit, S: Space> Rect<T, S> {
    /// The empty rectangle at the origin.
    pub const ZERO: Self = Self {
        origin: Point::ORIGIN,
        size: Size::ZERO,
    };

    /// Creates the smallest rectangle with the two given corners, in either order.
    ///
    /// ```
    /// use iceblink::geom::{Logical, LogicalPx, Point, Rect, Size};
    ///
    /// let rect: Rect<LogicalPx, Logical> = Rect::from_corners(
    ///     Point::new(LogicalPx(30.0), LogicalPx(10.0)),
    ///     Point::new(LogicalPx(10.0), LogicalPx(40.0)),
    /// );
    /// assert_eq!(rect.origin, Point::new(LogicalPx(10.0), LogicalPx(10.0)));
    /// assert_eq!(rect.size, Size::new(LogicalPx(20.0), LogicalPx(30.0)));
    /// ```
    #[inline]
    pub fn from_corners(first: Point<T, S>, second: Point<T, S>) -> Self {
        // Corners already in order are their own minimum and maximum, so this skips the four
        // comparisons below and gives the same bits.
        if first.x < second.x && first.y < second.y {
            return Self::new(first, second.offset_from(first));
        }

        let min = first.min(second);
        let max = first.max(second);
        Self::new(min, max.offset_from(min))
    }

    /// Returns the left edge.
    #[inline]
    pub fn left(self) -> T {
        self.origin.x
    }

    /// Returns the top edge.
    #[inline]
    pub fn top(self) -> T {
        self.origin.y
    }

    /// Returns the right edge, which is the left edge plus the width.
    #[inline]
    pub fn right(self) -> T {
        self.origin.x + self.size.width
    }

    /// Returns the bottom edge, which is the top edge plus the height.
    #[inline]
    pub fn bottom(self) -> T {
        self.origin.y + self.size.height
    }

    /// Returns the width.
    #[inline]
    pub fn width(self) -> T {
        self.size.width
    }

    /// Returns the height.
    #[inline]
    pub fn height(self) -> T {
        self.size.height
    }

    /// Returns the bottom-right corner.
    #[inline]
    pub fn far_corner(self) -> Point<T, S> {
        Point::new(self.right(), self.bottom())
    }

    /// Returns the four corners, clockwise from the top left.
    #[inline]
    pub fn corners(self) -> [Point<T, S>; 4] {
        [
            self.origin,
            Point::new(self.right(), self.top()),
            self.far_corner(),
            Point::new(self.left(), self.bottom()),
        ]
    }

    /// Reports whether an extent is zero or negative, so the rectangle contains no points.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.size.is_empty()
    }

    /// Reports whether `point` lies inside the rectangle.
    ///
    /// The left and top edges are inside and the right and bottom edges are outside, so adjacent
    /// rectangles share no point.
    #[inline]
    pub fn contains(self, point: Point<T, S>) -> bool {
        point.x >= self.left()
            && point.x < self.right()
            && point.y >= self.top()
            && point.y < self.bottom()
    }

    /// Reports whether `other` lies entirely within this rectangle.
    ///
    /// An empty `other` is contained by every rectangle.
    #[inline]
    pub fn contains_rect(self, other: Rect<T, S>) -> bool {
        if other.is_empty() {
            return true;
        }

        !self.is_empty()
            && other.left() >= self.left()
            && other.top() >= self.top()
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    /// Reports whether the two rectangles share any point.
    #[inline]
    pub fn intersects(self, other: Rect<T, S>) -> bool {
        self.intersection(other).is_some()
    }

    /// Returns the overlap of the two rectangles, or `None` if they do not overlap.
    #[inline]
    pub fn intersection(self, other: Rect<T, S>) -> Option<Self> {
        let origin = self.origin.max(other.origin);
        let far = self.far_corner().min(other.far_corner());
        let candidate = Self::new(origin, far.offset_from(origin));
        (far.x > origin.x && far.y > origin.y).then_some(candidate)
    }

    /// Returns the smallest rectangle containing both, ignoring an empty one.
    #[inline]
    pub fn union(self, other: Rect<T, S>) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }

        Self::from_corners(
            self.origin.min(other.origin),
            self.far_corner().max(other.far_corner()),
        )
    }

    /// Shrinks the rectangle by `edges`, stopping at zero extent.
    #[inline]
    pub fn inset(self, edges: Edges<T>) -> Self {
        let origin = Point::new(self.left() + edges.left, self.top() + edges.top);
        let size = Size::new(
            self.size.width - edges.horizontal(),
            self.size.height - edges.vertical(),
        );
        Self::new(origin, size.non_negative())
    }

    /// Grows the rectangle by `edges`.
    #[inline]
    pub fn outset(self, edges: Edges<T>) -> Self {
        let origin = Point::new(self.left() - edges.left, self.top() - edges.top);
        let size = Size::new(
            self.size.width + edges.horizontal(),
            self.size.height + edges.vertical(),
        );
        Self::new(origin, size)
    }

    /// Moves the rectangle by `offset`.
    #[inline]
    pub fn translate(self, offset: Size<T, S>) -> Self {
        Self::new(self.origin + offset, self.size)
    }

    /// Converts every coordinate to unit `U` through [`f32`].
    #[inline]
    pub fn to_unit<U: Unit>(self) -> Rect<U, S> {
        Rect::new(self.origin.to_unit(), self.size.to_unit())
    }
}
