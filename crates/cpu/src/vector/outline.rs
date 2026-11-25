//! Rounded clip links as outlines a path rasteriser fills.

use iceblink::{ClipLinkEntry, FrameClipShape, Matrix4};
use kurbo::{Affine, BezPath, PathEl, Point, Vec2};

/// The tangent distance of the control points of a cubic that approximates a quarter ellipse.
const KAPPA: f64 = 0.552_284_749_830_793_6;

/// How many line segments stand for a corner that is not elliptical.
const SEGMENTS: u32 = 16;

/// How far a flattened projected outline strays from its curves, in the outline's own units.
const TOLERANCE: f64 = 0.05;

/// The outline of a clip link and the transform that maps it to device space.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClipOutline {
    pub(crate) path: BezPath,
    pub(crate) transform: Affine,
}

impl ClipOutline {
    /// Returns the outline of `link`, measured through the world matrices `spaces`, or `None`
    /// for a mask link.
    ///
    /// Under a projective matrix the outline is flattened and mapped onto the device, with an
    /// identity transform. A link whose space slot lies past `spaces` is measured in device space.
    pub(crate) fn of(link: &ClipLinkEntry, spaces: &[Matrix4]) -> Option<Self> {
        let FrameClipShape::RoundedRect {
            rect,
            radii,
            shapes,
        } = &link.shape
        else {
            return None;
        };
        let path = rounded_rect(*rect, *radii, *shapes);
        let Some(matrix) = spaces.get(link.space as usize) else {
            return Some(Self {
                path,
                transform: Affine::IDENTITY,
            });
        };
        Some(match affine(matrix) {
            Some(transform) => Self { path, transform },
            None => Self {
                path: projected(&path, matrix),
                transform: Affine::IDENTITY,
            },
        })
    }
}

/// Returns the plane transform `matrix` embeds, or `None` if it is not two-dimensional.
pub(crate) fn affine(matrix: &Matrix4) -> Option<Affine> {
    let plane = matrix.to_affine2()?;
    let coefficients = [plane.a, plane.b, plane.c, plane.d, plane.tx, plane.ty];
    Some(Affine::new(coefficients.map(f64::from)))
}

fn projected(path: &BezPath, matrix: &Matrix4) -> BezPath {
    let map = |point: Point| {
        let [x, y, _] = matrix.transform_point(point.x as f32, point.y as f32, 0.0);
        Point::new(f64::from(x), f64::from(y))
    };
    let mut out = BezPath::new();
    kurbo::flatten(path.iter(), TOLERANCE, |element| match element {
        PathEl::MoveTo(point) => out.move_to(map(point)),
        PathEl::LineTo(point) => out.line_to(map(point)),
        PathEl::ClosePath => out.close_path(),
        // Flattening yields only the three elements above.
        PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
    });
    out
}

/// A superellipse quadrant at one corner of a rectangle.
struct Corner {
    center: Vec2,
    /// The unit step from the centre towards the rectangle's vertical edge.
    across: Vec2,
    /// The unit step from the centre towards the rectangle's horizontal edge.
    down: Vec2,
    radius: (f64, f64),
    exponent: f64,
}

impl Corner {
    fn on_horizontal(&self) -> Vec2 {
        self.center + self.down * self.radius.1
    }

    fn on_vertical(&self) -> Vec2 {
        self.center + self.across * self.radius.0
    }

    fn is_square(&self) -> bool {
        self.radius.0 <= 0.0 || self.radius.1 <= 0.0
    }

    // Appends the corner from the current point on one edge to the other edge.
    fn append(&self, path: &mut BezPath, from_horizontal: bool) {
        let (start, end) = if from_horizontal {
            (self.on_horizontal(), self.on_vertical())
        } else {
            (self.on_vertical(), self.on_horizontal())
        };
        if self.exponent == 2.0 {
            let (start_pull, end_pull) = if from_horizontal {
                (self.across * self.radius.0, self.down * self.radius.1)
            } else {
                (self.down * self.radius.1, self.across * self.radius.0)
            };
            path.curve_to(
                (start + start_pull * KAPPA).to_point(),
                (end + end_pull * KAPPA).to_point(),
                end.to_point(),
            );
            return;
        }
        for step in 1..=SEGMENTS {
            let fraction = f64::from(step) / f64::from(SEGMENTS);
            let turn = if from_horizontal {
                fraction
            } else {
                1.0 - fraction
            };
            path.line_to(self.at(turn * core::f64::consts::FRAC_PI_2).to_point());
        }
    }

    // The point at `angle`, from 0 on the horizontal edge to a quarter turn on the vertical one.
    fn at(&self, angle: f64) -> Vec2 {
        let power = 2.0 / self.exponent;
        let (sin, cos) = angle.sin_cos();
        self.center
            + self.across * (self.radius.0 * sin.abs().powf(power))
            + self.down * (self.radius.1 * cos.abs().powf(power))
    }
}

/// Returns the outline of `rect` with corner `radii` and `shapes`, clockwise from the top left.
///
/// Radii are clamped to at least zero, then scaled down uniformly so that the two radii along
/// each side fit within it.
pub(crate) fn rounded_rect(rect: [f32; 4], radii: [f32; 8], shapes: [f32; 4]) -> BezPath {
    let [x, y, width, height] = rect.map(f64::from);
    let (left, top, right, bottom) = (x, y, x + width, y + height);
    let radii = fitted(
        radii.map(|radius| f64::from(radius).max(0.0)),
        width,
        height,
    );
    let corner = |index: usize, center: (f64, f64), across: (f64, f64), down: (f64, f64)| Corner {
        center: Vec2::new(center.0, center.1),
        across: Vec2::new(across.0, across.1),
        down: Vec2::new(down.0, down.1),
        radius: (radii[index * 2], radii[index * 2 + 1]),
        exponent: f64::from(shapes[index]).clamp(0.01, 64.0),
    };
    let corners = [
        corner(
            0,
            (left + radii[0], top + radii[1]),
            (-1.0, 0.0),
            (0.0, -1.0),
        ),
        corner(
            1,
            (right - radii[2], top + radii[3]),
            (1.0, 0.0),
            (0.0, -1.0),
        ),
        corner(
            2,
            (right - radii[4], bottom - radii[5]),
            (1.0, 0.0),
            (0.0, 1.0),
        ),
        corner(
            3,
            (left + radii[6], bottom - radii[7]),
            (-1.0, 0.0),
            (0.0, 1.0),
        ),
    ];
    let mut path = BezPath::new();
    path.move_to(corners[0].on_horizontal().to_point());
    // Clockwise, the top-right and bottom-left corners start on a horizontal edge and the other
    // two on a vertical one.
    for (index, corner) in corners.iter().enumerate().skip(1).chain([(0, &corners[0])]) {
        let from_horizontal = index % 2 == 1;
        let start = if from_horizontal {
            corner.on_horizontal()
        } else {
            corner.on_vertical()
        };
        path.line_to(start.to_point());
        if !corner.is_square() {
            corner.append(&mut path, from_horizontal);
        }
    }
    path.close_path();
    path
}

fn fitted(radii: [f64; 8], width: f64, height: f64) -> [f64; 8] {
    let ratio = |side: f64, first: f64, second: f64| {
        let total = first + second;
        if total <= 0.0 {
            f64::INFINITY
        } else {
            (side / total).max(0.0)
        }
    };
    let scale = ratio(width, radii[0], radii[2])
        .min(ratio(height, radii[3], radii[5]))
        .min(ratio(width, radii[4], radii[6]))
        .min(ratio(height, radii[7], radii[1]));
    if scale.is_nan() || scale >= 1.0 {
        radii
    } else {
        radii.map(|radius| radius * scale)
    }
}
