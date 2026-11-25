//! Mapping a device point back onto the plane of a local space.

use iceblink::Matrix4;

/// Returns the point of the plane `z = 0` of a space with world matrix `world` that lands on the
/// device point `point`, or `None` when no point of the plane lands there in front of the viewer.
pub(crate) fn unproject(world: &Matrix4, point: [f32; 2]) -> Option<[f32; 2]> {
    let c = world.columns.map(|column| column.map(f64::from));
    let [px, py] = point.map(f64::from);

    // The plane maps through the columns of x, y and w, since z is zero on it.
    let h = [
        [c[0][0], c[1][0], c[3][0]],
        [c[0][1], c[1][1], c[3][1]],
        [c[0][3], c[1][3], c[3][3]],
    ];
    let inverse = invert(h)?;

    let local = [
        inverse[0][0] * px + inverse[0][1] * py + inverse[0][2],
        inverse[1][0] * px + inverse[1][1] * py + inverse[1][2],
        inverse[2][0] * px + inverse[2][1] * py + inverse[2][2],
    ];
    if local[2] == 0.0 {
        return None;
    }
    let (x, y) = (local[0] / local[2], local[1] / local[2]);

    // The point must lie in front of the viewer, with a positive homogeneous w.
    let w = h[2][0] * x + h[2][1] * y + h[2][2];
    if w.partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater) || !x.is_finite() || !y.is_finite()
    {
        return None;
    }
    Some([x as f32, y as f32])
}

/// Returns the inverse of a 3x3 matrix in rows, or `None` when it is singular.
fn invert(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let cofactor =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let det = m[0][0] * cofactor(1, 2, 1, 2) - m[0][1] * cofactor(1, 2, 0, 2)
        + m[0][2] * cofactor(1, 2, 0, 1);
    if det == 0.0 || !det.is_finite() {
        return None;
    }

    let inv = 1.0 / det;
    Some([
        [
            cofactor(1, 2, 1, 2) * inv,
            -cofactor(0, 2, 1, 2) * inv,
            cofactor(0, 1, 1, 2) * inv,
        ],
        [
            -cofactor(1, 2, 0, 2) * inv,
            cofactor(0, 2, 0, 2) * inv,
            -cofactor(0, 1, 0, 2) * inv,
        ],
        [
            cofactor(1, 2, 0, 1) * inv,
            -cofactor(0, 2, 0, 1) * inv,
            cofactor(0, 1, 0, 1) * inv,
        ],
    ])
}
