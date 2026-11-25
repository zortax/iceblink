//! Moving instanced frame kinds by a chunk offset.

use crate::frame::{ColorSprite, FrameMarks, FrameShadow, Sprite};
use crate::item::{Line, Quad, ShaderQuad};

/// An instanced frame kind, which a drawn chunk's offset moves.
pub trait Instance: Copy {
    /// Returns the instance moved by `by`, in its local units.
    ///
    /// The offset is added to the origins of the instance's rectangles, to its paint origin and
    /// to the payload origin of marks, and to nothing else.
    #[must_use]
    fn moved(self, by: [f32; 2]) -> Self;
}

/// Adds `by` to the origin of `rect`.
fn moved_rect(rect: [f32; 4], by: [f32; 2]) -> [f32; 4] {
    [rect[0] + by[0], rect[1] + by[1], rect[2], rect[3]]
}

/// Adds `by` to `point`.
fn moved_point(point: [f32; 2], by: [f32; 2]) -> [f32; 2] {
    [point[0] + by[0], point[1] + by[1]]
}

impl Instance for Quad {
    fn moved(mut self, by: [f32; 2]) -> Self {
        self.bounds = moved_rect(self.bounds, by);
        self.paint_origin = moved_point(self.paint_origin, by);
        self
    }
}

impl Instance for ShaderQuad {
    fn moved(mut self, by: [f32; 2]) -> Self {
        self.bounds = moved_rect(self.bounds, by);
        self.paint_origin = moved_point(self.paint_origin, by);
        self
    }
}

impl Instance for FrameShadow {
    fn moved(mut self, by: [f32; 2]) -> Self {
        self.bounds = moved_rect(self.bounds, by);
        self.shadow.shape_bounds = moved_rect(self.shadow.shape_bounds, by);
        self.shadow.caster_bounds = moved_rect(self.shadow.caster_bounds, by);
        self
    }
}

impl Instance for Line {
    fn moved(mut self, by: [f32; 2]) -> Self {
        self.bounds = moved_rect(self.bounds, by);
        self
    }
}

impl Instance for Sprite {
    fn moved(mut self, by: [f32; 2]) -> Self {
        self.bounds = moved_rect(self.bounds, by);
        self.paint_origin = moved_point(self.paint_origin, by);
        self
    }
}

impl Instance for ColorSprite {
    fn moved(mut self, by: [f32; 2]) -> Self {
        self.bounds = moved_rect(self.bounds, by);
        self.frame = moved_rect(self.frame, by);
        self.paint_origin = moved_point(self.paint_origin, by);
        self
    }
}

impl Instance for FrameMarks {
    fn moved(mut self, by: [f32; 2]) -> Self {
        self.marks.bounds = moved_rect(self.marks.bounds, by);
        self.marks.origin = moved_point(self.marks.origin, by);
        self.marks.paint_origin = moved_point(self.marks.paint_origin, by);
        self
    }
}
