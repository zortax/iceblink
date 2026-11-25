//! The signed distance to a rounded rectangle with superellipse corners.

use iceblink::{CornerShapes, LocalRect};

/// Returns the signed distance from the local point `p` to the rounded rectangle `rect`, negative
/// inside.
///
/// `radii` holds a horizontal and a vertical radius per corner, clockwise from the top left, and
/// `shapes` the exponent of each corner's superellipse.
pub(crate) fn rounded_distance(
    p: [f32; 2],
    rect: LocalRect,
    radii: [f32; 8],
    shapes: CornerShapes,
) -> f32 {
    let [x, y, width, height] = rect;
    let centre = [x + width / 2.0, y + height / 2.0];
    let half = [width / 2.0, height / 2.0];
    let left = p[0] < centre[0];
    let top = p[1] < centre[1];
    let corner = match (top, left) {
        (true, true) => 0,
        (true, false) => 1,
        (false, false) => 2,
        (false, true) => 3,
    };
    let (rx, ry, n) = (radii[corner * 2], radii[corner * 2 + 1], shapes[corner]);

    let q = [
        (p[0] - centre[0]).abs() - half[0] + rx,
        (p[1] - centre[1]).abs() - half[1] + ry,
    ];
    if q[0] <= 0.0 || q[1] <= 0.0 || rx <= 0.0 || ry <= 0.0 {
        return (q[0] - rx).max(q[1] - ry);
    }
    if n == 2.0 && rx == ry {
        return q[0].hypot(q[1]) - rx;
    }
    if n == 2.0 {
        let k1 = (q[0] / rx).hypot(q[1] / ry);
        let k2 = (q[0] / (rx * rx)).hypot(q[1] / (ry * ry));
        return k1 * (k1 - 1.0) / k2;
    }

    let a = [(q[0].abs() / rx).max(1e-6), (q[1].abs() / ry).max(1e-6)];
    let v = (a[0].powf(n) + a[1].powf(n)).powf(1.0 / n);
    let g = v.powf(1.0 - n) * (a[0].powf(n - 1.0) / rx).hypot(a[1].powf(n - 1.0) / ry);
    (v - 1.0) / g.max(1e-6)
}
