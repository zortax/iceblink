//! The determinant and inverse of a 4x4 matrix.

use crate::geom::transform::Matrix4;

impl Matrix4 {
    /// Returns the determinant.
    pub fn determinant(&self) -> f32 {
        (0..4)
            .map(|column| self.get(0, column) * self.cofactor(0, column))
            .sum()
    }

    /// Returns the signed cofactor of the entry at `row`, `column`.
    fn cofactor(&self, row: usize, column: usize) -> f32 {
        let mut minor = [[0.0_f32; 3]; 3];
        let mut target_row = 0;
        for source_row in 0..4 {
            if source_row == row {
                continue;
            }
            let mut target_column = 0;
            for source_column in 0..4 {
                if source_column == column {
                    continue;
                }
                minor[target_row][target_column] = self.get(source_row, source_column);
                target_column += 1;
            }
            target_row += 1;
        }

        let determinant = minor[0][0] * (minor[1][1] * minor[2][2] - minor[1][2] * minor[2][1])
            - minor[0][1] * (minor[1][0] * minor[2][2] - minor[1][2] * minor[2][0])
            + minor[0][2] * (minor[1][0] * minor[2][1] - minor[1][1] * minor[2][0]);

        if (row + column).is_multiple_of(2) {
            determinant
        } else {
            -determinant
        }
    }

    /// Returns the matrix that undoes this one, or `None` if the determinant is zero or not finite.
    pub fn invert(&self) -> Option<Self> {
        let determinant = self.determinant();
        if determinant == 0.0 || !determinant.is_finite() {
            return None;
        }

        let inverse = determinant.recip();
        // The adjugate is the transpose of the cofactor matrix.
        let columns = core::array::from_fn(|column| {
            core::array::from_fn(|row| self.cofactor(column, row) * inverse)
        });
        Some(Self::from_columns(columns))
    }
}
