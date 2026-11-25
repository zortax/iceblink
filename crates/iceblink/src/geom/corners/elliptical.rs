//! Elliptical corner radii.

use core::ops::{Add, Mul, Sub};

use crate::geom::corners::Corners;
use crate::geom::size::Size;
use crate::geom::space::Space;
use crate::geom::unit::Unit;

/// A pair of values along the two axes, horizontal first.
///
/// As a corner radius it holds the two semi-axes of the ellipse the corner is a quadrant of.
/// [`Vec2::splat`] creates the circular case. A pair carries no space.
///
/// ```
/// use iceblink::geom::{LogicalPx, Vec2};
///
/// let elliptical = Vec2::new(LogicalPx(20.0), LogicalPx(10.0));
/// let circular = Vec2::splat(LogicalPx(10.0));
/// assert!(!elliptical.is_uniform());
/// assert!(circular.is_uniform());
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Vec2<T> {
    /// The horizontal component.
    pub x: T,
    /// The vertical component.
    pub y: T,
}

impl<T> Vec2<T> {
    /// Creates a pair with the given components.
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }

    /// Applies `function` to both components, possibly changing the value type.
    pub fn map<U>(self, mut function: impl FnMut(T) -> U) -> Vec2<U> {
        Vec2 {
            x: function(self.x),
            y: function(self.y),
        }
    }

    /// Returns the two components, horizontal first.
    pub fn into_array(self) -> [T; 2] {
        [self.x, self.y]
    }
}

impl<T: Copy> Vec2<T> {
    /// Creates a pair with `value` on both axes, which as a corner radius is a circular corner.
    pub const fn splat(value: T) -> Self {
        Self { x: value, y: value }
    }
}

impl<T: Unit> Vec2<T> {
    /// Zero on both axes.
    pub const ZERO: Self = Self::splat(T::ZERO);

    /// Reports whether both components are equal.
    pub fn is_uniform(self) -> bool {
        self.x == self.y
    }

    /// Reports whether either component is zero or negative, which as a corner radius is a square
    /// corner.
    pub fn is_degenerate(self) -> bool {
        self.x <= T::ZERO || self.y <= T::ZERO
    }

    /// Converts both components to unit `U` through [`f32`].
    pub fn to_unit<U: Unit>(self) -> Vec2<U> {
        Vec2::new(U::from_f32(self.x.to_f32()), U::from_f32(self.y.to_f32()))
    }
}

impl<T: Unit> Add for Vec2<T> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl<T: Unit> Sub for Vec2<T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl<T: Unit> Mul<f32> for Vec2<T> {
    type Output = Self;

    fn mul(self, factor: f32) -> Self {
        self.map(|value| T::from_f32(value.to_f32() * factor))
    }
}

impl<T: Unit> Corners<Vec2<T>> {
    /// Reports whether every corner has equal horizontal and vertical radii.
    pub fn is_circular(self) -> bool {
        self.all(|radius| radius.is_uniform())
    }

    /// Reports whether every corner radius is degenerate, so every corner is square.
    pub fn is_square(self) -> bool {
        self.all(|radius| radius.is_degenerate())
    }

    /// Scales the radii down uniformly so that the two radii along each side of `size` fit within
    /// that side.
    ///
    /// Every radius is first clamped to at least zero. If the radii along some side add up to more
    /// than the side, every radius is then multiplied by the smallest ratio of side length to
    /// radius sum, so the corners keep their proportions. A side of negative length gives a ratio
    /// of zero, so every radius becomes zero. Radii that already fit are returned clamped and
    /// otherwise unchanged.
    ///
    /// ```
    /// use iceblink::geom::{Corners, Logical, LogicalPx, Size, Vec2};
    ///
    /// // Two radii of 60 along a side of 100 overflow it, so every radius shrinks by 100/120.
    /// let radii = Corners::uniform(Vec2::splat(LogicalPx(60.0)));
    /// let size: Size<LogicalPx, Logical> = Size::new(LogicalPx(100.0), LogicalPx(100.0));
    /// let fitted = radii.fit_within(size);
    /// assert_eq!(fitted.top_left, Vec2::splat(LogicalPx(50.0)));
    /// ```
    pub fn fit_within<S: Space>(self, size: Size<T, S>) -> Self {
        let width = size.width.to_f32();
        let height = size.height.to_f32();
        let clamped = self.map(|radius| Vec2::new(radius.x.max(T::ZERO), radius.y.max(T::ZERO)));

        let ratio = side_ratio(width, clamped.top_left.x, clamped.top_right.x)
            .min(side_ratio(
                height,
                clamped.top_right.y,
                clamped.bottom_right.y,
            ))
            .min(side_ratio(
                width,
                clamped.bottom_right.x,
                clamped.bottom_left.x,
            ))
            .min(side_ratio(
                height,
                clamped.bottom_left.y,
                clamped.top_left.y,
            ));

        // A NaN ratio leaves the radii alone too.
        if ratio.is_nan() || ratio >= 1.0 {
            clamped
        } else {
            clamped.map(|radius| radius * ratio)
        }
    }
}

/// Returns the ratio of a side's length to the sum of the two radii along it.
///
/// The ratio is infinite when the radii sum to zero, and zero for a side of negative length.
fn side_ratio<T: Unit>(side: f32, first: T, second: T) -> f32 {
    let total = first.to_f32() + second.to_f32();
    if total <= 0.0 {
        f32::INFINITY
    } else {
        (side / total).max(0.0)
    }
}
