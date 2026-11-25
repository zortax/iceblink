//! Folding what a pass's raster depends on into a hash.

use std::hash::{DefaultHasher, Hasher};
use std::sync::Arc;

use iceblink::{
    FramePaint, FramePath, GradientShape, PaintKind, PaintRef, PlannedPass, PlannedPath,
};

use crate::tables::table::FrameTable;

/// How outlines are folded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outlines {
    /// By the address of their `Arc`s.
    Addresses,
    /// By their elements.
    Elements,
}

/// Returns a hasher that has folded the region size of `pass` and, for every path of `planned`,
/// its outline and clip outlines, its matrix relative to the region's origin, its brush, fill
/// and stroke with their paints, and its ink relative to the region.
pub(super) fn fold_pass(
    pass: &PlannedPass,
    planned: &[PlannedPath],
    paths: &[FramePath],
    paints: &FrameTable<FramePaint>,
    outlines: Outlines,
) -> DefaultHasher {
    let mut fold = Fold(DefaultHasher::new());
    fold.int(pass.region.size.width);
    fold.int(pass.region.size.height);

    let origin = [f64::from(pass.region.left()), f64::from(pass.region.top())];
    for item in planned {
        let Some(path) = paths.get(item.path as usize) else {
            fold.int(-1);
            continue;
        };
        fold.outline(&path.path, outlines);
        for clip in path.clips.iter() {
            fold.outline(&clip.path, outlines);
            fold.int(i32::from(clip.rule.0));
        }

        let mut transform = path.transform.as_coeffs();
        transform[4] -= origin[0];
        transform[5] -= origin[1];
        fold.doubles(&transform);
        fold.doubles(&path.brush.as_coeffs());

        match &path.fill {
            Some(fill) => {
                fold.int(i32::from(fill.rule.0));
                fold.paint(fill.paint, paints);
            }
            None => fold.int(-1),
        }
        match &path.stroke {
            Some(stroke) => {
                fold.stroke(&stroke.style);
                fold.paint(stroke.paint, paints);
            }
            None => fold.int(-1),
        }

        fold.int(item.ink.left());
        fold.int(item.ink.top());
        fold.int(item.ink.size.width);
        fold.int(item.ink.size.height);
    }

    fold.0
}

struct Fold(DefaultHasher);

impl Fold {
    fn int(&mut self, value: i32) {
        self.0.write_i32(value);
    }

    fn floats(&mut self, values: &[f32]) {
        for value in values {
            self.0.write_u32(value.to_bits());
        }
    }

    fn doubles(&mut self, values: &[f64]) {
        for value in values {
            self.0.write_u64(value.to_bits());
        }
    }

    fn outline(&mut self, path: &Arc<kurbo::BezPath>, outlines: Outlines) {
        match outlines {
            Outlines::Addresses => self.0.write_usize(Arc::as_ptr(path) as usize),
            Outlines::Elements => {
                for element in path.elements() {
                    let zero = kurbo::Point::ZERO;
                    let (tag, points, count) = match *element {
                        kurbo::PathEl::MoveTo(a) => (0, [a, zero, zero], 1),
                        kurbo::PathEl::LineTo(a) => (1, [a, zero, zero], 1),
                        kurbo::PathEl::QuadTo(a, b) => (2, [a, b, zero], 2),
                        kurbo::PathEl::CurveTo(a, b, c) => (3, [a, b, c], 3),
                        kurbo::PathEl::ClosePath => (4, [zero; 3], 0),
                    };
                    self.int(tag);
                    for point in &points[..count] {
                        self.doubles(&[point.x, point.y]);
                    }
                }
            }
        }
    }

    fn stroke(&mut self, stroke: &kurbo::Stroke) {
        self.doubles(&[stroke.width, stroke.miter_limit, stroke.dash_offset]);
        self.int(stroke.join as i32);
        self.int(stroke.start_cap as i32);
        self.int(stroke.end_cap as i32);
        self.doubles(&stroke.dash_pattern);
    }

    fn paint(&mut self, reference: PaintRef, paints: &FrameTable<FramePaint>) {
        let paint = if reference.kind == PaintKind::NONE {
            None
        } else {
            paints.get(reference.index)
        };
        match paint {
            None | Some(FramePaint::None) => self.int(0),
            Some(FramePaint::Solid(color)) => {
                self.int(1);
                self.floats(&color.0);
            }
            Some(FramePaint::Sides(colors)) => {
                self.int(2);
                for color in colors {
                    self.floats(&color.0);
                }
            }
            Some(FramePaint::Gradient(gradient)) => {
                self.int(3);
                match gradient.shape {
                    GradientShape::Linear { start, end } => {
                        self.int(0);
                        self.floats(&start);
                        self.floats(&end);
                    }
                    GradientShape::Radial { center, radius } => {
                        self.int(1);
                        self.floats(&center);
                        self.floats(&radius);
                    }
                    GradientShape::Conic { center, angle } => {
                        self.int(2);
                        self.floats(&center);
                        self.floats(&[angle]);
                    }
                }
                self.int(i32::from(gradient.encoding.0));
                self.int(i32::from(gradient.repeat));
                for stop in gradient.stops.iter() {
                    self.floats(&stop.color);
                    self.floats(&[stop.offset]);
                }
            }
        }
    }
}
