//! The maps between a mark item's payload, its distance space, its local space and the device.

use iceblink::geom::{Device, Rect};
use iceblink::{MarkFlags, Marks};

use crate::draw::footprint::footprint_of;
use crate::tables::Space;

/// The maps from a mark item's distance space to its local space and the device.
#[derive(Clone, Copy, Debug)]
pub(super) struct Mapping<'a> {
    pub(super) space: &'a Space,
    axes: [f32; 4],
    inverse: [f32; 4],
    origin: [f32; 2],
    screen: bool,
    /// The length in distance units of the shortest device pixel.
    pub(super) margin: f32,
}

/// A device pixel's centre in every space a prim is evaluated in.
#[derive(Clone, Copy, Debug)]
pub(super) struct Sample {
    /// The point in distance space.
    pub(super) point: [f32; 2],
    /// How far one device pixel along x moves the distance-space point.
    pub(super) across: [f32; 2],
    /// How far one device pixel along y moves it.
    pub(super) down: [f32; 2],
    /// The point in the item's local space.
    pub(super) local: [f32; 2],
}

impl<'a> Mapping<'a> {
    /// Returns the mapping of `marks`, drawn in `space`.
    pub(super) fn of(marks: &Marks, space: &'a Space) -> Self {
        let [a, b, c, d] = marks.axes;
        let determinant = a * d - b * c;
        let inverse = if determinant == 0.0 {
            [0.0; 4]
        } else {
            [
                d / determinant,
                -b / determinant,
                -c / determinant,
                a / determinant,
            ]
        };
        let screen = marks.flags.contains(MarkFlags::SCREEN);
        let columns = space.matrix().columns;
        let x = [columns[0][0], columns[0][1]];
        let y = [columns[1][0], columns[1][1]];
        let margin = if screen {
            margin_of(x, y)
        } else {
            // The space's linear part after the axes.
            let first = [x[0] * a + y[0] * b, x[1] * a + y[1] * b];
            let second = [x[0] * c + y[0] * d, x[1] * c + y[1] * d];
            margin_of(first, second)
        };
        Self {
            space,
            axes: marks.axes,
            inverse,
            origin: marks.origin,
            screen,
            margin,
        }
    }

    /// Returns a payload position in distance space.
    pub(super) fn payload_point(&self, p: [f32; 2]) -> [f32; 2] {
        if self.screen {
            return self.map(p);
        }
        p
    }

    /// Returns the payload position `p` mapped by the axes and the origin, in local space.
    pub(super) fn map(&self, p: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d] = self.axes;
        [
            a * p[0] + c * p[1] + self.origin[0],
            b * p[0] + d * p[1] + self.origin[1],
        ]
    }

    /// Returns a distance-space point in local space.
    pub(super) fn local_point(&self, q: [f32; 2]) -> [f32; 2] {
        if self.screen {
            return q;
        }
        self.map(q)
    }

    /// Returns the device pixels of `within` that the distance-space quadrilateral `corners` can
    /// cover.
    pub(super) fn footprint(
        &self,
        corners: [[f32; 2]; 4],
        within: Rect<i32, Device>,
    ) -> Option<Rect<i32, Device>> {
        footprint_of(
            self.space,
            corners.map(|corner| self.local_point(corner)),
            within,
        )
    }

    /// Returns the device point `device` in every space, or `None` behind the viewer.
    pub(super) fn sample(&self, device: [f32; 2]) -> Option<Sample> {
        let local = self.space.local(device)?;
        let [dx, dy] = self.space.steps(device);
        if self.screen {
            return Some(Sample {
                point: local,
                across: dx,
                down: dy,
                local,
            });
        }
        let back = |v: [f32; 2]| {
            let [a, b, c, d] = self.inverse;
            [a * v[0] + c * v[1], b * v[0] + d * v[1]]
        };
        Some(Sample {
            point: back([local[0] - self.origin[0], local[1] - self.origin[1]]),
            across: back(dx),
            down: back(dy),
            local,
        })
    }
}

/// Returns one over the smallest singular value of the matrix with columns `x` and `y`, at most
/// 10⁴.
pub(super) fn margin_of(x: [f32; 2], y: [f32; 2]) -> f32 {
    let p = x[0] * x[0] + x[1] * x[1];
    let q = y[0] * y[0] + y[1] * y[1];
    let r = x[0] * y[0] + x[1] * y[1];
    let spread = (0.25 * (p - q) * (p - q) + r * r).max(0.0).sqrt();
    let smallest = (0.5 * (p + q) - spread).max(1e-12).sqrt();
    (1.0 / smallest).min(1e4)
}
