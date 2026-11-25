//! The two-dimensional affine transform.

use core::ops::Mul;

use crate::geom::point::Point;
use crate::geom::rect::Rect;
use crate::geom::space::Space;
use crate::geom::transform::Matrix4;
use crate::geom::unit::Unit;

/// A two-dimensional affine transform.
///
/// The six coefficients map a point as
///
/// ```text
/// x' = a * x + c * y + tx
/// y' = b * x + d * y + ty
/// ```
///
/// so `a`, `b`, `c` and `d` are the linear part and `tx`, `ty` the translation.
///
/// ```
/// use iceblink::geom::{Affine2, Logical, LogicalPx, Point};
///
/// let move_right = Affine2::translation(10.0, 0.0);
/// let double = Affine2::scale(2.0, 2.0);
///
/// let point: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.0), LogicalPx(1.0));
/// // `then` reads in application order: move first, then scale.
/// assert_eq!(
///     move_right.then(double).transform_point(point),
///     Point::new(LogicalPx(22.0), LogicalPx(2.0)),
/// );
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2 {
    /// The `x` coefficient of the transformed `x`.
    pub a: f32,
    /// The `x` coefficient of the transformed `y`.
    pub b: f32,
    /// The `y` coefficient of the transformed `x`.
    pub c: f32,
    /// The `y` coefficient of the transformed `y`.
    pub d: f32,
    /// The horizontal translation.
    pub tx: f32,
    /// The vertical translation.
    pub ty: f32,
}

impl Affine2 {
    /// The identity transform.
    pub const IDENTITY: Self = Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    /// Creates a transform from its six coefficients.
    pub const fn new(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> Self {
        Self { a, b, c, d, tx, ty }
    }

    /// Creates a translation.
    pub const fn translation(tx: f32, ty: f32) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, tx, ty)
    }

    /// Creates a scale about the origin.
    pub const fn scale(x: f32, y: f32) -> Self {
        Self::new(x, 0.0, 0.0, y, 0.0, 0.0)
    }

    /// Creates a rotation about the origin by `radians`, turning the x axis toward the y axis.
    ///
    /// With y growing downward, a positive angle turns clockwise on screen.
    pub fn rotation(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self::new(cos, sin, -sin, cos, 0.0, 0.0)
    }

    /// Creates a skew about the origin, with both angles in radians.
    pub fn skew(x_radians: f32, y_radians: f32) -> Self {
        Self::new(1.0, y_radians.tan(), x_radians.tan(), 1.0, 0.0, 0.0)
    }

    /// Returns this transform followed by `next`.
    ///
    /// ```
    /// use iceblink::geom::Affine2;
    ///
    /// let identity = Affine2::translation(3.0, 4.0).then(Affine2::translation(-3.0, -4.0));
    /// assert_eq!(identity, Affine2::IDENTITY);
    /// ```
    pub fn then(self, next: Self) -> Self {
        Self::new(
            next.a * self.a + next.c * self.b,
            next.b * self.a + next.d * self.b,
            next.a * self.c + next.c * self.d,
            next.b * self.c + next.d * self.d,
            next.a * self.tx + next.c * self.ty + next.tx,
            next.b * self.tx + next.d * self.ty + next.ty,
        )
    }

    /// Returns the determinant of the linear part, the factor areas are multiplied by.
    pub fn determinant(self) -> f32 {
        self.a * self.d - self.b * self.c
    }

    /// Returns the transform that undoes this one, or `None` if the determinant is zero or not
    /// finite.
    pub fn invert(self) -> Option<Self> {
        let determinant = self.determinant();
        if determinant == 0.0 || !determinant.is_finite() {
            return None;
        }

        let inverse = determinant.recip();
        let a = self.d * inverse;
        let b = -self.b * inverse;
        let c = -self.c * inverse;
        let d = self.a * inverse;
        Some(Self::new(
            a,
            b,
            c,
            d,
            -(a * self.tx + c * self.ty),
            -(b * self.tx + d * self.ty),
        ))
    }

    /// Applies the transform to a position, translation included.
    pub fn transform_point<T: Unit, S: Space>(self, point: Point<T, S>) -> Point<T, S> {
        let x = point.x.to_f32();
        let y = point.y.to_f32();
        Point::new(
            T::from_f32(self.a * x + self.c * y + self.tx),
            T::from_f32(self.b * x + self.d * y + self.ty),
        )
    }

    /// Applies only the linear part to the vector `(x, y)`.
    pub fn transform_vector<T: Unit>(self, x: T, y: T) -> (T, T) {
        let (x, y) = (x.to_f32(), y.to_f32());
        (
            T::from_f32(self.a * x + self.c * y),
            T::from_f32(self.b * x + self.d * y),
        )
    }

    /// Returns the smallest axis-aligned rectangle containing the four transformed corners of
    /// `rect`.
    ///
    /// ```
    /// use iceblink::geom::{Affine2, Logical, LogicalPx, Point, Rect, Size};
    ///
    /// let square: Rect<LogicalPx, Logical> = Rect::new(
    ///     Point::new(LogicalPx(0.0), LogicalPx(0.0)),
    ///     Size::new(LogicalPx(10.0), LogicalPx(10.0)),
    /// );
    /// let turned = Affine2::rotation(std::f32::consts::FRAC_PI_4).transform_rect(square);
    /// assert!(turned.width().0 > LogicalPx(14.1).0);
    /// ```
    pub fn transform_rect<T: Unit, S: Space>(self, rect: Rect<T, S>) -> Rect<T, S> {
        let [first, second, third, fourth] =
            rect.corners().map(|corner| self.transform_point(corner));
        let min = first.min(second).min(third).min(fourth);
        let max = first.max(second).max(third).max(fourth);
        Rect::from_corners(min, max)
    }

    /// Reports whether the linear part is the identity, so the transform only translates.
    ///
    /// ```
    /// use iceblink::geom::Affine2;
    ///
    /// assert!(Affine2::IDENTITY.is_translation());
    /// assert!(Affine2::translation(3.5, -2.0).is_translation());
    /// assert!(!Affine2::rotation(0.1).is_translation());
    /// assert!(!Affine2::scale(1.0, 2.0).is_translation());
    /// ```
    pub fn is_translation(self) -> bool {
        self.a == 1.0 && self.b == 0.0 && self.c == 0.0 && self.d == 1.0
    }

    /// Returns the same transform as a 4x4 matrix that leaves the z axis alone.
    pub fn to_matrix4(self) -> Matrix4 {
        Matrix4::from_columns([
            [self.a, self.b, 0.0, 0.0],
            [self.c, self.d, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [self.tx, self.ty, 0.0, 1.0],
        ])
    }
}

impl Default for Affine2 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Affine2 {
    type Output = Self;

    /// Composes two transforms in matrix order, so `self` is applied after `other`.
    fn mul(self, other: Self) -> Self {
        other.then(self)
    }
}
