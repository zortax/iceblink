//! One value per corner of a box.

use crate::geom::unit::Unit;

/// Four values, one per corner of a rectangle, in the order top-left, top-right, bottom-right,
/// bottom-left.
///
/// ```
/// use iceblink::geom::{Corners, LogicalPx, Vec2};
///
/// let radii = Corners::uniform(Vec2::splat(LogicalPx(8.0)));
/// assert!(radii.is_circular());
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Corners<T> {
    /// The top-left corner.
    pub top_left: T,
    /// The top-right corner.
    pub top_right: T,
    /// The bottom-right corner.
    pub bottom_right: T,
    /// The bottom-left corner.
    pub bottom_left: T,
}

impl<T> Corners<T> {
    /// Creates corners from values given clockwise from the top left.
    pub const fn new(top_left: T, top_right: T, bottom_right: T, bottom_left: T) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    /// Applies `function` to every corner, possibly changing the value type.
    pub fn map<U>(self, mut function: impl FnMut(T) -> U) -> Corners<U> {
        Corners {
            top_left: function(self.top_left),
            top_right: function(self.top_right),
            bottom_right: function(self.bottom_right),
            bottom_left: function(self.bottom_left),
        }
    }

    /// Returns the four values, clockwise from the top left.
    pub fn into_array(self) -> [T; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    /// Reports whether `predicate` holds for every corner.
    pub fn all(&self, mut predicate: impl FnMut(&T) -> bool) -> bool {
        predicate(&self.top_left)
            && predicate(&self.top_right)
            && predicate(&self.bottom_right)
            && predicate(&self.bottom_left)
    }

    /// Combines two sets of corners pairwise with `function`.
    pub fn zip_with<U, V>(
        self,
        other: Corners<U>,
        mut function: impl FnMut(T, U) -> V,
    ) -> Corners<V> {
        Corners {
            top_left: function(self.top_left, other.top_left),
            top_right: function(self.top_right, other.top_right),
            bottom_right: function(self.bottom_right, other.bottom_right),
            bottom_left: function(self.bottom_left, other.bottom_left),
        }
    }
}

impl<T: Copy> Corners<T> {
    /// Creates corners with `value` at every corner.
    pub const fn uniform(value: T) -> Self {
        Self {
            top_left: value,
            top_right: value,
            bottom_right: value,
            bottom_left: value,
        }
    }
}

impl<T: Unit> Corners<T> {
    /// Zero at every corner.
    pub const ZERO: Self = Self::uniform(T::ZERO);

    /// Reports whether every corner is zero.
    pub fn is_zero(self) -> bool {
        self.all(|value| *value == T::ZERO)
    }

    /// Returns the largest of the four values.
    pub fn largest(self) -> T {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }
}

#[cfg(test)]
mod tests {
    use super::Corners;
    use crate::geom::unit::LogicalPx;

    #[test]
    fn the_order_is_clockwise_from_the_top_left() {
        let corners = Corners::new(
            LogicalPx(1.0),
            LogicalPx(2.0),
            LogicalPx(3.0),
            LogicalPx(4.0),
        );
        assert_eq!(
            corners.into_array(),
            [
                LogicalPx(1.0),
                LogicalPx(2.0),
                LogicalPx(3.0),
                LogicalPx(4.0)
            ]
        );
    }

    #[test]
    fn largest_finds_the_maximum() {
        let corners = Corners::new(
            LogicalPx(1.0),
            LogicalPx(9.0),
            LogicalPx(3.0),
            LogicalPx(4.0),
        );
        assert_eq!(corners.largest(), LogicalPx(9.0));
        assert!(!corners.is_zero());
        assert!(Corners::<LogicalPx>::ZERO.is_zero());
    }
}
