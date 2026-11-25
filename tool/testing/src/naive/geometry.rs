//! Boxes, device rectangles and the matrix arithmetic of lowering.

use iceblink::geom::{Point, Size};
use iceblink::kurbo::Affine;
use iceblink::{DeviceRect, LocalRect, Matrix4};

/// An axis-aligned box between two corners, empty when a far edge is not past its near edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bounds {
    pub(crate) x0: f32,
    pub(crate) y0: f32,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
}

impl Bounds {
    pub(crate) const EMPTY: Self = Self {
        x0: f32::INFINITY,
        y0: f32::INFINITY,
        x1: f32::NEG_INFINITY,
        y1: f32::NEG_INFINITY,
    };

    pub(crate) fn of(rect: LocalRect) -> Self {
        Self {
            x0: rect[0],
            y0: rect[1],
            x1: rect[0] + rect[2],
            y1: rect[1] + rect[3],
        }
    }

    pub(crate) fn of_device(rect: DeviceRect) -> Self {
        Self {
            x0: rect.origin.x as f32,
            y0: rect.origin.y as f32,
            x1: rect.origin.x as f32 + rect.size.width as f32,
            y1: rect.origin.y as f32 + rect.size.height as f32,
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        // Written so that NaN edges count as empty.
        !(self.x1 > self.x0 && self.y1 > self.y0)
    }

    pub(crate) fn union(self, other: Self) -> Self {
        if other.is_empty() {
            return self;
        }

        if self.is_empty() {
            return other;
        }

        Self {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    pub(crate) fn intersect(self, other: Self) -> Self {
        Self {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        }
    }

    pub(crate) fn outset(self, by: f32) -> Self {
        Self {
            x0: self.x0 - by,
            y0: self.y0 - by,
            x1: self.x1 + by,
            y1: self.y1 + by,
        }
    }

    pub(crate) fn translated(self, by: [f32; 2]) -> Self {
        Self {
            x0: self.x0 + by[0],
            y0: self.y0 + by[1],
            x1: self.x1 + by[0],
            y1: self.y1 + by[1],
        }
    }

    /// Returns the box as `[x, y, width, height]`, with zero size when it is empty.
    pub(crate) fn to_rect(self) -> [f32; 4] {
        if self.is_empty() {
            return [0.0; 4];
        }

        [self.x0, self.y0, self.x1 - self.x0, self.y1 - self.y0]
    }

    /// Returns the whole device pixels the box touches, or `None` when it is empty.
    pub(crate) fn round_out(self) -> Option<DeviceRect> {
        if self.is_empty() {
            return None;
        }

        let x0 = whole(self.x0.floor());
        let y0 = whole(self.y0.floor());
        let x1 = whole(self.x1.ceil());
        let y1 = whole(self.y1.ceil());
        Some(device_rect(x0, y0, x1 - x0, y1 - y0))
    }

    /// Maps the box's corners through `matrix` and returns their bounding box, or `None` when a
    /// corner has a homogeneous `w` of zero or below.
    pub(crate) fn mapped(self, matrix: &Matrix4) -> Option<Self> {
        if self.is_empty() {
            return Some(Self::EMPTY);
        }

        let corners = [
            (self.x0, self.y0),
            (self.x1, self.y0),
            (self.x1, self.y1),
            (self.x0, self.y1),
        ];
        let mut mapped = Self::EMPTY;
        for (x, y) in corners {
            let [px, py] = map_point(matrix, x, y)?;
            mapped.x0 = mapped.x0.min(px);
            mapped.y0 = mapped.y0.min(py);
            mapped.x1 = mapped.x1.max(px);
            mapped.y1 = mapped.y1.max(py);
        }

        Some(mapped)
    }
}

/// Converts a coordinate to a pixel index, saturating at a quarter of the `i32` range so that
/// widths stay representable.
fn whole(value: f32) -> i32 {
    const LIMIT: f32 = (1 << 29) as f32;
    value.clamp(-LIMIT, LIMIT) as i32
}

pub(crate) fn device_rect(x: i32, y: i32, width: i32, height: i32) -> DeviceRect {
    DeviceRect::new(Point::new(x, y), Size::new(width, height))
}

/// Returns the device rectangle of an output of `size`.
pub(crate) fn output_rect(size: [u32; 2]) -> DeviceRect {
    device_rect(0, 0, size[0] as i32, size[1] as i32)
}

/// Maps a local point through `matrix`, or returns `None` when its `w` is zero or below.
pub(crate) fn map_point(matrix: &Matrix4, x: f32, y: f32) -> Option<[f32; 2]> {
    let [px, py, _, w] = matrix.transform_vector4([x, y, 0.0, 1.0]);
    if w > 0.0 {
        Some([px / w, py / w])
    } else {
        None
    }
}

/// Returns the plane part of `matrix` as an affine map, when it maps the plane affinely.
pub(crate) fn plane_affine(matrix: &Matrix4) -> Option<Affine> {
    let c = &matrix.columns;
    if c[0][3] != 0.0 || c[1][3] != 0.0 || c[3][3] <= 0.0 {
        return None;
    }

    let w = f64::from(c[3][3]);
    Some(Affine::new([
        f64::from(c[0][0]) / w,
        f64::from(c[0][1]) / w,
        f64::from(c[1][0]) / w,
        f64::from(c[1][1]) / w,
        f64::from(c[3][0]) / w,
        f64::from(c[3][1]) / w,
    ]))
}

/// Returns a translation of the plane.
pub(crate) fn translation(by: [f32; 2]) -> Matrix4 {
    Matrix4::translation(by[0], by[1], 0.0)
}
