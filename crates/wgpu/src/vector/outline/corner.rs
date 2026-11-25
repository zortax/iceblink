//! One rounded corner of a rectangle.

use kurbo::{BezPath, Vec2};

/// The tangent distance of the control points of a cubic that approximates a quarter ellipse.
// Puts the curve's midpoint on the ellipse, with a radial error near one part in ten thousand.
const KAPPA: f64 = 0.552_284_749_830_793_6;

/// How many line segments stand for a corner that is not elliptical.
const SEGMENTS: u32 = 16;

/// A superellipse quadrant `|x/rx|^n + |y/ry|^n = 1` at one corner of a rectangle.
pub(super) struct Corner {
    /// The centre of the superellipse.
    pub(super) center: Vec2,
    /// The unit step from the centre towards the rectangle's vertical edge.
    pub(super) across: Vec2,
    /// The unit step from the centre towards the rectangle's horizontal edge.
    pub(super) down: Vec2,
    /// The horizontal and vertical radius.
    pub(super) radius: (f64, f64),
    /// The exponent `n`, clamped to `[0.01, 64]`.
    pub(super) exponent: f64,
}

impl Corner {
    /// Returns the point where the corner meets the rectangle's horizontal edge.
    pub(super) fn on_horizontal(&self) -> Vec2 {
        self.center + self.down * self.radius.1
    }

    /// Returns the point where the corner meets the rectangle's vertical edge.
    pub(super) fn on_vertical(&self) -> Vec2 {
        self.center + self.across * self.radius.0
    }

    /// Reports whether the corner is square.
    pub(super) fn is_square(&self) -> bool {
        self.radius.0 <= 0.0 || self.radius.1 <= 0.0
    }

    /// Appends the corner to `path`, from the current point on one edge to the other edge.
    ///
    /// The corner runs from the horizontal edge to the vertical one when `from_horizontal` is
    /// set, and the other way otherwise.
    pub(super) fn append(&self, path: &mut BezPath, from_horizontal: bool) {
        let (start, end) = if from_horizontal {
            (self.on_horizontal(), self.on_vertical())
        } else {
            (self.on_vertical(), self.on_horizontal())
        };
        if self.exponent == 2.0 {
            let (start_pull, end_pull) = if from_horizontal {
                (self.across * self.radius.0, self.down * self.radius.1)
            } else {
                (self.down * self.radius.1, self.across * self.radius.0)
            };
            path.curve_to(
                (start + start_pull * KAPPA).to_point(),
                (end + end_pull * KAPPA).to_point(),
                end.to_point(),
            );
            return;
        }
        for step in 1..=SEGMENTS {
            let fraction = f64::from(step) / f64::from(SEGMENTS);
            let angle = if from_horizontal {
                fraction
            } else {
                1.0 - fraction
            } * core::f64::consts::FRAC_PI_2;
            path.line_to(self.at(angle).to_point());
        }
    }

    /// Returns the point at `angle`, which runs from 0 on the horizontal edge to a quarter turn
    /// on the vertical edge.
    fn at(&self, angle: f64) -> Vec2 {
        let power = 2.0 / self.exponent;
        let (sin, cos) = angle.sin_cos();
        self.center
            + self.across * (self.radius.0 * sin.abs().powf(power))
            + self.down * (self.radius.1 * cos.abs().powf(power))
    }
}
