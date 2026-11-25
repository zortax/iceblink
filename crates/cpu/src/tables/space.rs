//! A frame space: a world matrix and the map from device points back onto its plane.

use iceblink::Matrix4;

/// A world matrix with the inverse of its plane homography.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Space {
    matrix: Matrix4,
    /// The homography from device points `(x, y, 1)` to the plane, by rows.
    inverse: [[f64; 3]; 3],
    translation: Option<[f32; 2]>,
    projective: bool,
}

impl Space {
    /// Returns the space of `matrix`.
    ///
    /// A matrix that maps the plane to a line sends every device point far outside any shape.
    pub(crate) fn of(matrix: &Matrix4) -> Self {
        let c = &matrix.columns;
        Self {
            matrix: *matrix,
            inverse: plane_inverse(matrix),
            translation: matrix.plane_translation().map(|(x, y)| [x, y]),
            projective: c[0][3] != 0.0 || c[1][3] != 0.0 || c[3][3] != 1.0,
        }
    }

    /// Returns the world matrix.
    pub(crate) fn matrix(&self) -> &Matrix4 {
        &self.matrix
    }

    /// Returns the plane translation of a space that only translates.
    pub(crate) fn translation(&self) -> Option<[f32; 2]> {
        self.translation
    }

    /// Returns the point of the plane that maps to `device`, or `None` when that point lies
    /// behind the viewer.
    pub(crate) fn local(&self, device: [f32; 2]) -> Option<[f32; 2]> {
        let local = self.plane(device);
        if self.projective && self.w_of(local) <= 0.0 {
            return None;
        }
        Some(local)
    }

    /// Returns the point of the plane that maps to `device`, wherever it lies.
    pub(crate) fn plane(&self, device: [f32; 2]) -> [f32; 2] {
        if let Some([x, y]) = self.translation {
            return [device[0] - x, device[1] - y];
        }
        let [x, y] = [f64::from(device[0]), f64::from(device[1])];
        let h = &self.inverse;
        let u = h[0][0] * x + h[0][1] * y + h[0][2];
        let v = h[1][0] * x + h[1][1] * y + h[1][2];
        let w = h[2][0] * x + h[2][1] * y + h[2][2];
        [(u / w) as f32, (v / w) as f32]
    }

    /// Returns the device point `local` maps to, or `None` when its `w` is zero or below.
    pub(crate) fn device(&self, local: [f32; 2]) -> Option<[f32; 2]> {
        let c = &self.matrix.columns;
        let [x, y] = local;
        let w = c[0][3] * x + c[1][3] * y + c[3][3];
        if w <= 0.0 {
            return None;
        }
        Some([
            (c[0][0] * x + c[1][0] * y + c[3][0]) / w,
            (c[0][1] * x + c[1][1] * y + c[3][1]) / w,
        ])
    }

    /// Returns the device point `local` maps to, dividing by whatever `w` it has.
    pub(crate) fn device_unchecked(&self, local: [f32; 2]) -> [f32; 2] {
        let c = &self.matrix.columns;
        let [x, y] = local;
        let w = c[0][3] * x + c[1][3] * y + c[3][3];
        [
            (c[0][0] * x + c[1][0] * y + c[3][0]) / w,
            (c[0][1] * x + c[1][1] * y + c[3][1]) / w,
        ]
    }

    /// Returns how far one device pixel along x and along y moves the plane point at `device`.
    pub(crate) fn steps(&self, device: [f32; 2]) -> [[f32; 2]; 2] {
        if self.translation.is_some() {
            return [[1.0, 0.0], [0.0, 1.0]];
        }
        let [x, y] = [f64::from(device[0]), f64::from(device[1])];
        let h = &self.inverse;
        let u = h[0][0] * x + h[0][1] * y + h[0][2];
        let v = h[1][0] * x + h[1][1] * y + h[1][2];
        let w = h[2][0] * x + h[2][1] * y + h[2][2];
        let step = |column: usize| {
            let du = (h[0][column] * w - u * h[2][column]) / (w * w);
            let dv = (h[1][column] * w - v * h[2][column]) / (w * w);
            [du as f32, dv as f32]
        };
        [step(0), step(1)]
    }

    fn w_of(&self, local: [f32; 2]) -> f32 {
        let c = &self.matrix.columns;
        c[0][3] * local[0] + c[1][3] * local[1] + c[3][3]
    }
}

// The homography of the plane z = 0 takes columns 0, 1 and 3 and rows 0, 1 and 3.
fn plane_inverse(matrix: &Matrix4) -> [[f64; 3]; 3] {
    let c = &matrix.columns;
    let h = [
        [f64::from(c[0][0]), f64::from(c[1][0]), f64::from(c[3][0])],
        [f64::from(c[0][1]), f64::from(c[1][1]), f64::from(c[3][1])],
        [f64::from(c[0][3]), f64::from(c[1][3]), f64::from(c[3][3])],
    ];
    let cofactor =
        |r0: usize, r1: usize, c0: usize, c1: usize| h[r0][c0] * h[r1][c1] - h[r0][c1] * h[r1][c0];
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
        return [[0.0, 0.0, 1.0e9], [0.0, 0.0, 1.0e9], [0.0, 0.0, 1.0]];
    }
    adjugate.map(|row| row.map(|value| value / determinant))
}

#[cfg(test)]
mod tests {
    use iceblink::Matrix4;

    use super::Space;

    #[test]
    fn a_scale_and_move_maps_back_exactly() {
        let matrix = Matrix4::scale(2.0, 4.0, 1.0).then(&Matrix4::translation(8.0, 8.0, 0.0));
        let space = Space::of(&matrix);
        let device = space.device([3.0, 5.0]).expect("in front");
        let local = space.local(device).expect("in front");
        assert!((local[0] - 3.0).abs() < 1e-5 && (local[1] - 5.0).abs() < 1e-5);
        assert_eq!(space.steps(device), [[0.5, 0.0], [0.0, 0.25]]);
    }

    #[test]
    fn a_degenerate_matrix_sends_points_far_away() {
        let space = Space::of(&Matrix4::scale(0.0, 1.0, 1.0));
        let local = space.local([1.0, 1.0]).expect("affine");
        assert!(local[0] >= 1.0e8);
    }
}
