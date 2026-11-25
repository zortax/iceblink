//! Collecting a face's outline commands into a path in device space.

use iceblink::kurbo::{BezPath, Point};
use skrifa::outline::OutlinePen;

/// Writes a face's outline into a path with y down, leaning each point by the slant.
pub(super) struct Pen<'a> {
    pub(super) path: &'a mut BezPath,
    /// How far a point moves right per pixel above the baseline: the tangent of the slant.
    pub(super) shear: f32,
}

impl Pen<'_> {
    fn point(&self, x: f32, y: f32) -> Point {
        // Faces grow up from the baseline, and device space grows down.
        Point::new(f64::from(x + self.shear * y), f64::from(-y))
    }
}

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let to = self.point(x, y);
        self.path.move_to(to);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let to = self.point(x, y);
        self.path.line_to(to);
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (control, to) = (self.point(cx, cy), self.point(x, y));
        self.path.quad_to(control, to);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (first, second, to) = (self.point(cx0, cy0), self.point(cx1, cy1), self.point(x, y));
        self.path.curve_to(first, second, to);
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}
