//! The device pixels where a quad's shape covers differently after a change of its geometry.

use iceblink::{CornerShapes, LocalRect, Matrix4};

use crate::geometry::bounds::axis_scale;
use crate::geometry::coverage::CoverageChange;
use crate::geometry::coverage::compare::compare;
use crate::geometry::coverage::outline::Outline;

/// The geometry of a quad that decides its coverage: its outer shape and its border widths.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct QuadShape {
    /// The bounds, with the chunk offset applied.
    pub(crate) bounds: LocalRect,
    pub(crate) radii: [f32; 8],
    pub(crate) shapes: CornerShapes,
    /// The border widths, as `[top, right, bottom, left]`.
    pub(crate) border: [f32; 4],
}

impl QuadShape {
    /// Returns the inner shape of the border: the bounds inset by the widths, with each corner's
    /// radii reduced by the widths of its two sides.
    fn inner(&self) -> (LocalRect, [f32; 8]) {
        let [x, y, width, height] = self.bounds;
        let [top, right, bottom, left] = self.border;
        let rect = [
            x + left,
            y + top,
            width - left - right,
            height - top - bottom,
        ];
        // Each corner's horizontal radius shrinks by its vertical side, and its vertical radius
        // by its horizontal side.
        let sides = [[left, top], [right, top], [right, bottom], [left, bottom]];
        let mut radii = [0.0; 8];
        for (corner, [across, down]) in sides.into_iter().enumerate() {
            radii[corner * 2] = (self.radii[corner * 2] - across).max(0.0);
            radii[corner * 2 + 1] = (self.radii[corner * 2 + 1] - down).max(0.0);
        }
        (rect, radii)
    }
}

/// Returns the pixels where a quad drawn with `old` and with `new` under `world` can differ,
/// when its paints are the same and constant over the plane.
///
/// Returns `None` when the change cannot be bounded this way: a world matrix that turns or
/// projects, a negative size, or radii that break the overlap constraint.
pub(crate) fn quad_change(
    old: &QuadShape,
    new: &QuadShape,
    world: &Matrix4,
) -> Option<CoverageChange> {
    let scale = axis_scale(world)?;

    let mut change = CoverageChange::default();
    let (old_inner, old_inner_radii) = old.inner();
    let (new_inner, new_inner_radii) = new.inner();
    let outer = ((old.bounds, old.radii), (new.bounds, new.radii));
    let inner = ((old_inner, old_inner_radii), (new_inner, new_inner_radii));
    // Without borders the inner shapes are the outer ones, which find the same regions.
    let pairs = if inner == outer {
        &[outer][..]
    } else {
        &[outer, inner][..]
    };
    for &((old_rect, old_radii), (new_rect, new_radii)) in pairs {
        let before = Outline::antialiased(old_rect, old_radii, old.shapes, scale)?;
        let after = Outline::antialiased(new_rect, new_radii, new.shapes, scale)?;
        compare(&before, &after, &mut |found| change.push(found));
    }
    Some(change)
}
