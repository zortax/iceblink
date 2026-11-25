//! Rounded rectangles normalised for comparing their coverage.

use iceblink::{CornerShapes, LocalRect};

use crate::geometry::bounds::DeviceBox;
use crate::geometry::clip::RAMP;

/// A rounded rectangle with its corners clockwise from the top left, in one coordinate frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Outline {
    /// The edges as `[left, top, right, bottom]`.
    pub(super) edges: [f32; 4],
    /// The horizontal and vertical radius of each corner.
    pub(super) radii: [[f32; 2]; 4],
    pub(super) shapes: CornerShapes,
    /// How far coverage ramps on either side of a vertical and of a horizontal edge, zero for a
    /// hard edge.
    pub(super) ramp: [f32; 2],
}

impl Outline {
    /// Returns the device outline of a rounded rectangle in a space that maps `x` to
    /// `scale[0] x + scale[2]` and `y` to `scale[1] y + scale[3]`.
    ///
    /// A rectangle without radii has hard edges. Returns `None` for a rectangle of negative size
    /// or radii that break the overlap constraint.
    pub(super) fn on_device(
        rect: LocalRect,
        radii: [f32; 8],
        shapes: CornerShapes,
        scale: [f32; 4],
    ) -> Option<Self> {
        let [sx, sy, tx, ty] = scale;
        let local = Self::local(rect, radii, shapes)?;
        let [left, top, right, bottom] = local.edges;
        let (x0, x1) = (sx * left + tx, sx * right + tx);
        let (y0, y1) = (sy * top + ty, sy * bottom + ty);

        // A negative scale swaps the corners on that axis.
        let mut corners = [[0.0; 2]; 4];
        let mut exponents = [0.0; 4];
        for corner in 0..4 {
            let (right, bottom) = side(corner);
            let to = index(right != (sx < 0.0), bottom != (sy < 0.0));
            let [rx, ry] = local.radii[corner];
            corners[to] = [rx * sx.abs(), ry * sy.abs()];
            exponents[to] = local.shapes[corner];
        }

        let hard = radii.iter().all(|&radius| radius == 0.0);
        let ramp = if hard {
            [0.0; 2]
        } else {
            [RAMP * sx.abs(), RAMP * sy.abs()]
        };
        Some(Self {
            edges: [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)],
            radii: corners,
            shapes: exponents,
            ramp,
        })
    }

    /// Returns the device outline of an antialiased rounded rectangle, whose coverage ramps over
    /// half a device pixel on either side of every edge, in a space that scales and translates
    /// as for [`Outline::on_device`].
    pub(super) fn antialiased(
        rect: LocalRect,
        radii: [f32; 8],
        shapes: CornerShapes,
        scale: [f32; 4],
    ) -> Option<Self> {
        let mut outline = Self::on_device(rect, radii, shapes, scale)?;
        outline.ramp = [RAMP; 2];
        Some(outline)
    }

    /// Returns the outline of a rectangle without radii in its own space, with soft edges.
    pub(super) fn in_space(rect: LocalRect) -> Option<Self> {
        Self::local(rect, [0.0; 8], [2.0; 4])
    }

    fn local(rect: LocalRect, radii: [f32; 8], shapes: CornerShapes) -> Option<Self> {
        let [x, y, width, height] = rect;
        if width < 0.0 || height < 0.0 {
            return None;
        }

        let corners = [
            [radii[0], radii[1]],
            [radii[2], radii[3]],
            [radii[4], radii[5]],
            [radii[6], radii[7]],
        ];
        let fits = corners[0][0] + corners[1][0] <= width
            && corners[1][1] + corners[2][1] <= height
            && corners[2][0] + corners[3][0] <= width
            && corners[3][1] + corners[0][1] <= height;
        fits.then_some(Self {
            edges: [x, y, x + width, y + height],
            radii: corners,
            shapes,
            ramp: [RAMP; 2],
        })
    }

    /// Returns the rectangle grown by the ramp, outside which coverage is zero.
    pub(super) fn bounds(&self) -> DeviceBox {
        let [x0, y0, x1, y1] = self.edges;
        let [rx, ry] = self.ramp;
        DeviceBox {
            x0: x0 - rx,
            y0: y0 - ry,
            x1: x1 + rx,
            y1: y1 + ry,
        }
    }

    /// Returns the centre on axis `axis`, 0 for `x` and 1 for `y`.
    pub(super) fn centre(&self, axis: usize) -> f32 {
        (self.edges[axis] + self.edges[axis + 2]) * 0.5
    }

    /// Returns the region where corner `corner` decides the coverage, if the corner is rounded.
    ///
    /// The region reaches outwards without bound and inwards by the corner's radii.
    pub(super) fn zone(&self, corner: usize) -> Option<DeviceBox> {
        let [rx, ry] = self.radii[corner];
        if rx <= 0.0 || ry <= 0.0 {
            return None;
        }

        let [x0, y0, x1, y1] = self.edges;
        let (right, bottom) = side(corner);
        let (left_edge, right_edge) = if right {
            (x1 - rx, f32::INFINITY)
        } else {
            (f32::NEG_INFINITY, x0 + rx)
        };
        let (top_edge, bottom_edge) = if bottom {
            (y1 - ry, f32::INFINITY)
        } else {
            (f32::NEG_INFINITY, y0 + ry)
        };
        Some(DeviceBox {
            x0: left_edge,
            y0: top_edge,
            x1: right_edge,
            y1: bottom_edge,
        })
    }

    /// Returns the point where the two edges of corner `corner` meet.
    pub(super) fn corner_point(&self, corner: usize) -> [f32; 2] {
        let (right, bottom) = side(corner);
        let [x0, y0, x1, y1] = self.edges;
        [if right { x1 } else { x0 }, if bottom { y1 } else { y0 }]
    }
}

/// Returns whether corner `corner` lies on the right and whether it lies at the bottom.
pub(super) fn side(corner: usize) -> (bool, bool) {
    match corner {
        0 => (false, false),
        1 => (true, false),
        2 => (true, true),
        _ => (false, true),
    }
}

/// Returns the corner on the right or left and at the bottom or top.
fn index(right: bool, bottom: bool) -> usize {
    match (right, bottom) {
        (false, false) => 0,
        (true, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    }
}
