//! Corner shapes.

/// Superellipse exponents for the four corners, clockwise from the top left.
///
/// A corner is the superellipse quadrant `|x/rx|^n + |y/ry|^n = 1`. Exponent 2 is the ellipse, 1 a
/// bevel and 4 a squircle. Values below 1 scoop the corner inwards, and 64 closes it on the radius
/// box. Exponents are clamped to `[0.01, 64]` where they are evaluated.
pub type CornerShapes = [f32; 4];

/// Elliptical corners on all four sides.
pub const ROUND: CornerShapes = [2.0; 4];
