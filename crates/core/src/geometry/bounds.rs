//! Axis-aligned boxes in device space, mapping local rectangles onto the device, and the matrices
//! that keep the axes of the plane.

use iceblink::geom::{Point, Rect, Size};
use iceblink::{DeviceRect, LocalRect, Matrix4};

/// An axis-aligned box as `[left, top, right, bottom]`, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DeviceBox {
    pub(crate) x0: f32,
    pub(crate) y0: f32,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
}

impl DeviceBox {
    /// The box that contains every point.
    pub(crate) const EVERYWHERE: Self = Self {
        x0: f32::NEG_INFINITY,
        y0: f32::NEG_INFINITY,
        x1: f32::INFINITY,
        y1: f32::INFINITY,
    };

    /// Returns the box of a `[x, y, width, height]` rectangle.
    #[inline]
    pub(crate) fn of(rect: LocalRect) -> Self {
        Self {
            x0: rect[0],
            y0: rect[1],
            x1: rect[0] + rect[2],
            y1: rect[1] + rect[3],
        }
    }

    /// Reports whether the box holds no area.
    #[inline]
    pub(crate) fn is_empty(self) -> bool {
        !(self.x1 > self.x0 && self.y1 > self.y0)
    }

    #[inline]
    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    #[inline]
    pub(crate) fn intersect(self, other: Self) -> Self {
        Self {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        }
    }

    /// Returns the box grown by `[left, top, right, bottom]`.
    #[inline]
    pub(crate) fn outset(self, by: [f32; 4]) -> Self {
        Self {
            x0: self.x0 - by[0],
            y0: self.y0 - by[1],
            x1: self.x1 + by[2],
            y1: self.y1 + by[3],
        }
    }

    /// Returns the box as `[x, y, width, height]`.
    #[inline]
    pub(crate) fn rect(self) -> [f32; 4] {
        [self.x0, self.y0, self.x1 - self.x0, self.y1 - self.y0]
    }

    /// Returns the whole pixels the box touches, cut to `limit`, or `None` when none are left.
    #[inline]
    pub(crate) fn pixels(self, limit: DeviceRect) -> Option<DeviceRect> {
        if self.is_empty() {
            return None;
        }
        let x0 = clamp(
            self.x0.floor(),
            limit.origin.x,
            limit.origin.x + limit.size.width,
        );
        let y0 = clamp(
            self.y0.floor(),
            limit.origin.y,
            limit.origin.y + limit.size.height,
        );
        let x1 = clamp(
            self.x1.ceil(),
            limit.origin.x,
            limit.origin.x + limit.size.width,
        );
        let y1 = clamp(
            self.y1.ceil(),
            limit.origin.y,
            limit.origin.y + limit.size.height,
        );
        (x1 > x0 && y1 > y0).then(|| Rect::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0)))
    }
}

/// The largest rectangle ink is cut to before any output sees it.
pub(crate) const INK_LIMIT: DeviceRect = Rect::new(
    Point::new(-(1 << 24), -(1 << 24)),
    Size::new(1 << 25, 1 << 25),
);

fn clamp(value: f32, low: i32, high: i32) -> i32 {
    if value.is_nan() {
        return low;
    }
    value.clamp(low as f32, high as f32) as i32
}

/// Maps a local rectangle through `matrix` and returns the bounding box of its corners.
///
/// The box is everywhere when a corner maps to a `w` that is at or below zero or not finite.
pub(crate) fn map(matrix: &Matrix4, rect: LocalRect) -> DeviceBox {
    let local = DeviceBox::of(rect);
    if let Some((x, y)) = matrix.plane_translation() {
        return local.outset([-x, -y, x, y]);
    }

    let corners = [
        (local.x0, local.y0),
        (local.x1, local.y0),
        (local.x1, local.y1),
        (local.x0, local.y1),
    ];
    let mut mapped = DeviceBox {
        x0: f32::INFINITY,
        y0: f32::INFINITY,
        x1: f32::NEG_INFINITY,
        y1: f32::NEG_INFINITY,
    };
    for (x, y) in corners {
        let [px, py, _, w] = matrix.transform_vector4([x, y, 0.0, 1.0]);
        if w <= 0.0 || !w.is_finite() {
            return DeviceBox::EVERYWHERE;
        }
        let (px, py) = (px / w, py / w);
        mapped.x0 = mapped.x0.min(px);
        mapped.y0 = mapped.y0.min(py);
        mapped.x1 = mapped.x1.max(px);
        mapped.y1 = mapped.y1.max(py);
    }

    mapped
}

/// Returns `world` applied after a move by `offset`.
pub(crate) fn after_offset(world: &Matrix4, offset: [f32; 2]) -> Matrix4 {
    Matrix4::translation(offset[0], offset[1], 0.0).then(world)
}

/// Returns a `DeviceRect` as a box.
pub(crate) fn of_pixels(rect: DeviceRect) -> DeviceBox {
    DeviceBox {
        x0: rect.origin.x as f32,
        y0: rect.origin.y as f32,
        x1: (rect.origin.x + rect.size.width) as f32,
        y1: (rect.origin.y + rect.size.height) as f32,
    }
}

/// Returns the plane map `[sx, sy, tx, ty]` of a matrix that scales and translates the plane
/// without projecting, where `x' = sx x + tx` and `y' = sy y + ty`.
pub(crate) fn axis_scale(matrix: &Matrix4) -> Option<[f32; 4]> {
    let c = &matrix.columns;
    let w = c[3][3];
    if c[0][3] != 0.0 || c[1][3] != 0.0 || w <= 0.0 {
        return None;
    }

    let [sx, skew_y, skew_x, sy] = [c[0][0], c[0][1], c[1][0], c[1][1]];
    let scales = skew_y == 0.0 && skew_x == 0.0 && sx != 0.0 && sy != 0.0;
    scales.then(|| [sx / w, sy / w, c[3][0] / w, c[3][1] / w])
}

/// Reports whether a matrix keeps the axes of the plane or turns them by quarter turns, without
/// projecting.
pub(crate) fn keeps_axes(matrix: &Matrix4) -> bool {
    let c = &matrix.columns;
    let turns = c[0][0] == 0.0 && c[1][1] == 0.0 && c[0][1] != 0.0 && c[1][0] != 0.0;
    let plane = c[0][3] == 0.0 && c[1][3] == 0.0 && c[3][3] > 0.0;
    axis_scale(matrix).is_some() || (plane && turns)
}
