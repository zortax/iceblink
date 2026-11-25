//! The pixels near the edges and corners of clip links that the links cover only in part.

use iceblink::{ClipShape, DeviceRect, Matrix4};

use crate::geometry::bounds::{DeviceBox, INK_LIMIT, axis_scale, of_pixels};
use crate::geometry::clip::hard_edge;
use crate::geometry::coverage::outline::Outline;

/// Where a set of clip links covers a region they hold only in part: inside the ramps of soft
/// edges and inside the zones of rounded corners.
///
/// Every value is a device coordinate that a band or zone reaches inwards to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Partial {
    /// The reach of the ramps inside the left, top, right and bottom edges.
    sides: [f32; 4],
    /// The inner corner of each corner's zone, clockwise from the top left.
    corners: [[f32; 2]; 4],
}

impl Partial {
    /// The links that cover every pixel they hold.
    pub(crate) const NONE: Self = Self {
        sides: [
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::INFINITY,
        ],
        corners: [
            [f32::NEG_INFINITY, f32::NEG_INFINITY],
            [f32::INFINITY, f32::NEG_INFINITY],
            [f32::INFINITY, f32::INFINITY],
            [f32::NEG_INFINITY, f32::INFINITY],
        ],
    };

    /// Returns where a clip link of `shape` in a space with world matrix `world` covers in part,
    /// or `None` when that cannot be bounded, as for a mask or a rounded link in a turned space.
    pub(crate) fn of_link(world: &Matrix4, shape: &ClipShape) -> Option<Self> {
        if hard_edge(world, shape) {
            return Some(Self::NONE);
        }
        let ClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        } = shape
        else {
            return None;
        };

        let outline = Outline::on_device(*rect, *radii, *shapes, axis_scale(world)?)?;
        let [x0, y0, x1, y1] = outline.edges;
        let [rx, ry] = outline.ramp;
        let mut partial = Self {
            sides: [x0 + rx, y0 + ry, x1 - rx, y1 - ry],
            ..Self::NONE
        };

        for (corner, point) in partial.corners.iter_mut().enumerate() {
            if let Some(zone) = outline.zone(corner) {
                *point = match corner {
                    0 => [zone.x1, zone.y1],
                    1 => [zone.x0, zone.y1],
                    2 => [zone.x0, zone.y0],
                    _ => [zone.x1, zone.y0],
                };
            }
        }
        Some(partial)
    }

    /// Returns where either set of links covers in part.
    pub(crate) fn and(self, other: Self) -> Self {
        let [a, b] = [self.sides, other.sides];
        let [c, d] = [self.corners, other.corners];
        Self {
            sides: [
                a[0].max(b[0]),
                a[1].max(b[1]),
                a[2].min(b[2]),
                a[3].min(b[3]),
            ],
            corners: [
                [c[0][0].max(d[0][0]), c[0][1].max(d[0][1])],
                [c[1][0].min(d[1][0]), c[1][1].max(d[1][1])],
                [c[2][0].min(d[2][0]), c[2][1].min(d[2][1])],
                [c[3][0].max(d[3][0]), c[3][1].min(d[3][1])],
            ],
        }
    }

    /// Returns the whole pixels of `region` that the links may cover in part, when they hold all
    /// of `region`.
    pub(crate) fn pixels(self, region: DeviceRect) -> impl Iterator<Item = DeviceRect> {
        let open = DeviceBox::EVERYWHERE;
        let [left, top, right, bottom] = self.sides;
        let [tl, tr, br, bl] = self.corners;
        let boxes = [
            DeviceBox { x1: left, ..open },
            DeviceBox { y1: top, ..open },
            DeviceBox { x0: right, ..open },
            DeviceBox { y0: bottom, ..open },
            DeviceBox {
                x1: tl[0],
                y1: tl[1],
                ..open
            },
            DeviceBox {
                x0: tr[0],
                y1: tr[1],
                ..open
            },
            DeviceBox {
                x0: br[0],
                y0: br[1],
                ..open
            },
            DeviceBox {
                x1: bl[0],
                y0: bl[1],
                ..open
            },
        ];
        let region = of_pixels(region);
        boxes
            .into_iter()
            .filter_map(move |held| held.intersect(region).pixels(INK_LIMIT))
    }
}
