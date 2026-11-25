//! Encoding world matrices for the space table.

use iceblink::Matrix4;

use crate::bind::tables::records::GpuSpatial;

impl GpuSpatial {
    /// Encodes `matrix` and the inverse of its plane homography.
    ///
    /// A matrix that maps the plane to a line inverts to a map that sends every point far outside
    /// any clip.
    pub(crate) fn of(matrix: &Matrix4) -> Self {
        Self {
            matrix: matrix.columns,
            inverse: plane_inverse(matrix),
            flags: [u32::from(translation(matrix).is_some()), 0, 0, 0],
        }
    }
}

impl Default for GpuSpatial {
    fn default() -> Self {
        Self::of(&Matrix4::IDENTITY)
    }
}

/// Returns the plane translation of `matrix`, or `None` if it does more than translate the plane.
pub(crate) fn translation(matrix: &Matrix4) -> Option<[f32; 2]> {
    matrix.plane_translation().map(|(x, y)| [x, y])
}

/// Returns the plane map `[a, b, c, d, tx, ty]` of `matrix`, where `x' = a x + c y + tx` and
/// `y' = b x + d y + ty`, or `None` unless the matrix keeps the axes or turns them by quarter turns
/// without projecting.
pub(crate) fn axis_aligned(matrix: &Matrix4) -> Option<[f32; 6]> {
    let c = &matrix.columns;
    let w = c[3][3];
    if c[0][3] != 0.0 || c[1][3] != 0.0 || w <= 0.0 {
        return None;
    }
    let [a, b, cx, d] = [c[0][0], c[0][1], c[1][0], c[1][1]];
    let scales = b == 0.0 && cx == 0.0 && a != 0.0 && d != 0.0;
    let turns = a == 0.0 && d == 0.0 && b != 0.0 && cx != 0.0;
    (scales || turns).then(|| [a / w, b / w, cx / w, d / w, c[3][0] / w, c[3][1] / w])
}

/// Returns the inverse of the homography `matrix` gives the plane z = 0, by columns.
// The homography takes columns 0, 1 and 3 and rows 0, 1 and 3 of the matrix.
fn plane_inverse(matrix: &Matrix4) -> [[f32; 4]; 3] {
    let c = &matrix.columns;
    let h = [
        [f64::from(c[0][0]), f64::from(c[1][0]), f64::from(c[3][0])],
        [f64::from(c[0][1]), f64::from(c[1][1]), f64::from(c[3][1])],
        [f64::from(c[0][3]), f64::from(c[1][3]), f64::from(c[3][3])],
    ];
    let cofactor =
        |r0: usize, r1: usize, c0: usize, c1: usize| h[r0][c0] * h[r1][c1] - h[r0][c1] * h[r1][c0];
    // Rows of the adjugate.
    let adjugate = [
        [
            cofactor(1, 2, 1, 2),
            -cofactor(0, 2, 1, 2),
            cofactor(0, 1, 1, 2),
        ],
        [
            -cofactor(1, 2, 0, 2),
            cofactor(0, 2, 0, 2),
            -cofactor(0, 1, 0, 2),
        ],
        [
            cofactor(1, 2, 0, 1),
            -cofactor(0, 2, 0, 1),
            cofactor(0, 1, 0, 1),
        ],
    ];
    let determinant =
        h[0][0] * adjugate[0][0] + h[0][1] * adjugate[1][0] + h[0][2] * adjugate[2][0];
    if !determinant.is_finite() || determinant.abs() < 1.0e-12 {
        return [
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
            [1.0e9, 1.0e9, 1.0, 0.0],
        ];
    }
    // WGSL reads the matrix by columns.
    let column = |index: usize| {
        [
            (adjugate[0][index] / determinant) as f32,
            (adjugate[1][index] / determinant) as f32,
            (adjugate[2][index] / determinant) as f32,
            0.0,
        ]
    };
    [column(0), column(1), column(2)]
}

#[cfg(test)]
mod tests {
    use iceblink::Matrix4;

    use super::plane_inverse;

    fn apply(inverse: &[[f32; 4]; 3], x: f32, y: f32) -> (f32, f32) {
        let v = [
            inverse[0][0] * x + inverse[1][0] * y + inverse[2][0],
            inverse[0][1] * x + inverse[1][1] * y + inverse[2][1],
            inverse[0][2] * x + inverse[1][2] * y + inverse[2][2],
        ];
        (v[0] / v[2], v[1] / v[2])
    }

    #[test]
    fn a_translation_inverts_to_the_opposite_move() {
        let inverse = plane_inverse(&Matrix4::translation(10.0, -4.0, 0.0));
        assert_eq!(apply(&inverse, 15.0, 6.0), (5.0, 10.0));
    }

    #[test]
    fn a_scale_and_move_inverts_exactly() {
        let matrix = Matrix4::scale(2.0, 4.0, 1.0).then(&Matrix4::translation(8.0, 8.0, 0.0));
        let inverse = plane_inverse(&matrix);
        let [x, y, _] = matrix.transform_point(3.0, 5.0, 0.0);
        let (u, v) = apply(&inverse, x, y);
        assert!((u - 3.0).abs() < 1e-5 && (v - 5.0).abs() < 1e-5, "{u} {v}");
    }

    #[test]
    fn a_degenerate_matrix_sends_points_far_away() {
        let inverse = plane_inverse(&Matrix4::scale(0.0, 1.0, 1.0));
        let (u, _) = apply(&inverse, 1.0, 1.0);
        assert!(u >= 1.0e8);
    }
}
