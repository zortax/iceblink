//! Rounded rectangles as outlines.

use iceblink::{CornerShapes, LocalRect};
use kurbo::{BezPath, Vec2};

use super::corner::Corner;

/// Returns the outline of `rect` with the corner `radii` and corner `shapes` of a clip link.
///
/// `radii` holds a horizontal and a vertical radius per corner, clockwise from the top left, and
/// `shapes` holds the superellipse exponent of each corner in the same order. Radii are clamped to
/// at least zero, and then scaled down uniformly so that the two radii along each side fit within
/// it. An exponent of 2 gives an elliptical corner. The outline runs clockwise from the top left.
pub fn rounded_rect(rect: LocalRect, radii: [f32; 8], shapes: CornerShapes) -> BezPath {
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

/// Returns `radii` scaled down uniformly so that the two radii along each side of a `width` by
/// `height` rectangle fit within that side.
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
