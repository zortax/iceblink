//! Placements of mark shapes too large for a cell, lowered into filled paths.

use std::sync::Arc;

use iceblink::geom::Matrix4;
use iceblink::{FillRule, FramePath, MarkFlags, MarkPayload, Marks, PathClip, PathFill};
use kurbo::{Affine, BezPath, Point, Rect, Shape};

use crate::content::path::project::project;
use crate::content::path::{affine_of, device_ink};

/// Appends paths for the placements `marks` draws whose shape is marked in `large`, under the
/// world matrix `world`.
///
/// Each placement fills its shape with the non-zero rule at its anchor, quantised to a quarter
/// pixel, cut to the item's bounds. Under [`MarkFlags::UNION`] the placements form one path, so
/// their overlaps blend once. Otherwise each placement is a path of its own. A placement whose
/// anchor has a homogeneous `w` of zero or below is not drawn.
pub(crate) fn large_paths(
    marks: &Marks,
    payload: &MarkPayload,
    large: &[bool],
    world: &Matrix4,
    out: &mut Vec<FramePath>,
) {
    let first = marks.first as usize;
    let Some(placements) = payload
        .placements
        .get(first..first + marks.placements as usize)
    else {
        return;
    };

    let view = View::of(marks, world);
    let [a, b, c, d] = marks.axes.map(f64::from);
    let [ox, oy] = marks.origin.map(f64::from);
    let placed = placements.iter().filter_map(|placement| {
        let shape = placement.shape as usize;
        let (true, Some(path)) = (large.get(shape) == Some(&true), payload.shapes.get(shape))
        else {
            return None;
        };
        let (x, y) = (f64::from(placement.x), f64::from(placement.y));
        let local = Point::new(a * x + c * y + ox, b * x + d * y + oy);
        let anchor = view.device(local)?;
        let quantised = (
            (anchor.x * 4.0).round() / 4.0,
            (anchor.y * 4.0).round() / 4.0,
        );
        Some(Placed {
            path,
            local,
            anchor,
            at: Affine::translate(quantised),
        })
    });

    if marks.flags.contains(MarkFlags::UNION) {
        let mut union = BezPath::new();
        let mut first = None;
        for mark in placed {
            union.extend(
                mark.path
                    .elements()
                    .iter()
                    .map(|element| mark.at * *element),
            );
            first.get_or_insert((mark.local, mark.anchor));
        }
        if let Some(pin) = first {
            view.push(marks, Arc::new(union), Affine::IDENTITY, pin, out);
        }
        return;
    }
    for mark in placed {
        let pin = (mark.local, mark.anchor);
        view.push(marks, Arc::clone(mark.path), mark.at, pin, out);
    }
}

/// A placement of a large shape: its local and device anchor, and where its path is drawn.
struct Placed<'a> {
    path: &'a Arc<BezPath>,
    local: Point,
    anchor: Point,
    /// The map from the shape's space to the device, which moves it to the quantised anchor.
    at: Affine,
}

/// An item's bounds on the device, and the map from its local points to device points.
struct View {
    bounds: BezPath,
    /// The world matrix as an affine map, or `None` when it projects.
    affine: Option<Affine>,
    world: Matrix4,
}

impl View {
    fn of(marks: &Marks, world: &Matrix4) -> Self {
        let [x, y, width, height] = marks.bounds.map(f64::from);
        let local = Rect::new(x, y, x + width, y + height).to_path(0.1);
        let affine = world.to_affine2().map(affine_of);
        let bounds = match affine {
            Some(affine) => affine * local,
            None => project(&local, Affine::IDENTITY, world),
        };
        Self {
            bounds,
            affine,
            world: *world,
        }
    }

    /// Returns the device point of `local`, or `None` where it is not drawn.
    fn device(&self, local: Point) -> Option<Point> {
        if let Some(affine) = self.affine {
            return Some(affine * local);
        }

        let [x, y, _, w] = self
            .world
            .transform_vector4([local.x as f32, local.y as f32, 0.0, 1.0]);
        (w > 0.0).then(|| Point::new(f64::from(x / w), f64::from(y / w)))
    }

    /// Appends `path`, mapped to the device by `transform`, filled with the item's paint and cut
    /// to its bounds.
    ///
    /// The paint is evaluated in the item's local space under an affine world matrix. Under a
    /// projective one, local space is taken as the device moved so that the local point of `pin`
    /// maps to its device point.
    fn push(
        &self,
        marks: &Marks,
        path: Arc<BezPath>,
        transform: Affine,
        pin: (Point, Point),
        out: &mut Vec<FramePath>,
    ) {
        let to_path = transform.inverse();
        let clip = to_path * self.bounds.clone();
        let ink = device_ink(path.control_box().intersect(clip.control_box()), transform);
        if ink.size.width <= 0 || ink.size.height <= 0 {
            return;
        }

        let paint_origin = Affine::translate((
            f64::from(marks.paint_origin[0]),
            f64::from(marks.paint_origin[1]),
        ));
        let (local, anchor) = pin;
        let to_device = self
            .affine
            .unwrap_or_else(|| Affine::translate(anchor - local));
        out.push(FramePath {
            path,
            transform,
            brush: to_path * to_device * paint_origin,
            fill: Some(PathFill {
                paint: marks.paint,
                rule: FillRule::NON_ZERO,
            }),
            stroke: None,
            clips: Arc::from([PathClip {
                path: Arc::new(clip),
                rule: FillRule::NON_ZERO,
            }]),
            ink,
        });
    }
}
