//! The damage of a re-recorded quad that changed only its shape.

use iceblink::{BorderStyle, ItemKind, PaintKind, PaintRef, Quad, SpaceRef};

use crate::geometry::coverage::{CoverageChange, QuadShape, quad_change};
use crate::scene::Scene;
use crate::scene::chunk::ChunkRecord;

/// Returns where the quads at `position` of `old` and `new` can draw differently, when they
/// differ only in their bounds, radii, corner shapes or border widths and paint constant colours.
///
/// Returns `None` for every other difference, and when the quad's world matrix turns or
/// projects. The chunk must not have moved between the two records.
pub(super) fn reshaped(
    scene: &Scene,
    old: &ChunkRecord,
    new: &ChunkRecord,
    position: usize,
) -> Option<CoverageChange> {
    let (a, b) = (
        old.recorded.ops.get(position)?,
        new.recorded.ops.get(position)?,
    );
    if a.kind != ItemKind::QUAD || b.kind != ItemKind::QUAD {
        return None;
    }
    let before = old.recorded.quads.get(a.index as usize)?;
    let after = new.recorded.quads.get(b.index as usize)?;
    if !same_paint(before, after) || old.offset != new.offset {
        return None;
    }
    let slot = if after.space == SpaceRef::INHERIT {
        new.recorded.props.space.slot()
    } else {
        after.space.0
    };
    let world = scene.transforms.at(slot)?.world;
    quad_change(
        &shape(before, old.offset),
        &shape(after, new.offset),
        &world,
    )
}

/// Reports whether two quads paint alike everywhere they both cover, with constant colours.
fn same_paint(a: &Quad, b: &Quad) -> bool {
    let constant = |paint: PaintRef| {
        matches!(
            paint.kind,
            PaintKind::NONE | PaintKind::SOLID | PaintKind::INLINE
        )
    };
    let undashed = |quad: &Quad| {
        quad.style == BorderStyle::SOLID || quad.border.iter().all(|&width| width == 0.0)
    };
    a.fill == b.fill
        && a.stroke == b.stroke
        && a.clip == b.clip
        && a.space == b.space
        && constant(a.fill)
        && constant(a.stroke)
        && undashed(a)
        && undashed(b)
}

/// Returns the shape of `quad` moved by the chunk offset.
fn shape(quad: &Quad, offset: [f32; 2]) -> QuadShape {
    let [x, y, width, height] = quad.bounds;
    QuadShape {
        bounds: [x + offset[0], y + offset[1], width, height],
        radii: quad.radii,
        shapes: quad.shapes,
        border: quad.border,
    }
}
