//! Conversions between rotation bases and unit quaternions.

use crate::geom::transform::Matrix4;

/// Returns the rotation matrix of a unit quaternion `[x, y, z, w]`.
pub(super) fn rotation_matrix([x, y, z, w]: [f32; 4]) -> Matrix4 {
    Matrix4::from_columns([
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
            0.0,
        ],
        [
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
            0.0,
        ],
        [
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Returns the unit quaternion of an orthonormal basis held as three column vectors, the one of
/// the two whose `w` is not negative.
pub(super) fn quaternion_of(basis: &[[f32; 3]; 3]) -> [f32; 4] {
    let quaternion = any_quaternion_of(basis);
    if quaternion[3] < 0.0 {
        quaternion.map(|component| -component)
    } else {
        quaternion
    }
}

/// Returns one of the two unit quaternions of an orthonormal basis held as three column vectors.
fn any_quaternion_of(basis: &[[f32; 3]; 3]) -> [f32; 4] {
    // Only the largest component comes from the diagonal and the rest from off-diagonal sums.
    // Taking all four from the diagonal loses a rotation of 1e-4 radians to cancellation in `f32`.
    let trace = basis[0][0] + basis[1][1] + basis[2][2];
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        return [
            (basis[1][2] - basis[2][1]) / s,
            (basis[2][0] - basis[0][2]) / s,
            (basis[0][1] - basis[1][0]) / s,
            0.25 * s,
        ];
    }
    if basis[0][0] > basis[1][1] && basis[0][0] > basis[2][2] {
        let s = (1.0 + basis[0][0] - basis[1][1] - basis[2][2]).sqrt() * 2.0;
        return [
            0.25 * s,
            (basis[1][0] + basis[0][1]) / s,
            (basis[2][0] + basis[0][2]) / s,
            (basis[1][2] - basis[2][1]) / s,
        ];
    }
    if basis[1][1] > basis[2][2] {
        let s = (1.0 + basis[1][1] - basis[0][0] - basis[2][2]).sqrt() * 2.0;
        return [
            (basis[1][0] + basis[0][1]) / s,
            0.25 * s,
            (basis[2][1] + basis[1][2]) / s,
            (basis[2][0] - basis[0][2]) / s,
        ];
    }
    let s = (1.0 + basis[2][2] - basis[0][0] - basis[1][1]).sqrt() * 2.0;
    [
        (basis[2][0] + basis[0][2]) / s,
        (basis[2][1] + basis[1][2]) / s,
        0.25 * s,
        (basis[0][1] - basis[1][0]) / s,
    ]
}
