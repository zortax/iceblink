//! Whether a local point lies inside an item's shape.

use iceblink::kurbo::{self, Shape, StrokeOpts};
use iceblink::{FillRule, ItemKind, ItemOp, PathItem, RecordedChunk};

use crate::geometry::ink::local_ink;
use crate::hit::distance::rounded_distance;
use crate::scene::chunk::RunInk;

/// How closely a stroke's band is flattened, in path units.
const STROKE_TOLERANCE: f64 = 0.05;

/// Reports whether the local point `p` lies inside the shape of the item `op` of `chunk`:
///
/// - A quad or shader quad's rounded rectangle, border included.
/// - An image's rounded frame, within the picture's extent.
/// - A path's fill by its rule or its stroke's band, inside every path clip.
/// - The local ink rectangle of a glyph run, line, marks item, external quad or backdrop.
///
/// A shadow is never hit.
pub(crate) fn contains(
    chunk: &RecordedChunk,
    op: ItemOp,
    runs: &[Option<RunInk>],
    p: [f32; 2],
) -> bool {
    let index = op.index as usize;
    match op.kind {
        ItemKind::QUAD => chunk
            .quads
            .get(index)
            .is_some_and(|quad| rounded_distance(p, quad.bounds, quad.radii, quad.shapes) <= 0.0),
        ItemKind::SHADER_QUAD => chunk
            .shader_quads
            .get(index)
            .is_some_and(|quad| rounded_distance(p, quad.bounds, quad.radii, quad.shapes) <= 0.0),
        ItemKind::IMAGE => chunk.images.get(index).is_some_and(|image| {
            rounded_distance(p, image.frame, image.radii, image.shapes) <= 0.0
                && in_ink(chunk, op, runs, p)
        }),
        ItemKind::PATH => chunk.paths.get(index).is_some_and(|path| in_path(path, p)),
        ItemKind::SHADOW => false,
        _ => in_ink(chunk, op, runs, p),
    }
}

/// Reports whether `p` lies inside the item's local ink rectangle, edges included.
fn in_ink(chunk: &RecordedChunk, op: ItemOp, runs: &[Option<RunInk>], p: [f32; 2]) -> bool {
    local_ink(chunk, op, runs).is_some_and(|ink| {
        let [x, y, width, height] = ink.rect;
        x <= p[0] && p[0] <= x + width && y <= p[1] && p[1] <= y + height
    })
}

/// Reports whether `p` lies inside a path's fill or stroke and inside every path clip.
fn in_path(path: &PathItem, p: [f32; 2]) -> bool {
    let point = path.placement.inverse() * kurbo::Point::new(f64::from(p[0]), f64::from(p[1]));
    if !point.x.is_finite() || !point.y.is_finite() {
        return false;
    }
    let filled = path
        .fill
        .is_some_and(|fill| encloses(path.path.winding(point), fill.rule));
    let stroked = || {
        path.stroke.as_ref().is_some_and(|stroke| {
            let band = kurbo::stroke(
                path.path.iter(),
                &stroke.style,
                &StrokeOpts::default(),
                STROKE_TOLERANCE,
            );
            band.winding(point) != 0
        })
    };
    (filled || stroked())
        && path
            .clips
            .iter()
            .all(|clip| encloses(clip.path.winding(point), clip.rule))
}

/// Reports whether a winding number counts as inside by `rule`.
fn encloses(winding: i32, rule: FillRule) -> bool {
    if rule == FillRule::EVEN_ODD {
        winding % 2 != 0
    } else {
        winding != 0
    }
}
