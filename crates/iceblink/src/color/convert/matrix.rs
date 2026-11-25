//! Row-major 3×3 matrices applied to a colour's three channels.

/// A row-major 3×3 matrix.
pub(crate) type Matrix3 = [[f32; 3]; 3];

/// Applies `matrix` to `vector`.
pub(crate) fn apply(matrix: &Matrix3, vector: [f32; 3]) -> [f32; 3] {
    [
        matrix[0][0] * vector[0] + matrix[0][1] * vector[1] + matrix[0][2] * vector[2],
        matrix[1][0] * vector[0] + matrix[1][1] * vector[1] + matrix[1][2] * vector[2],
        matrix[2][0] * vector[0] + matrix[2][1] * vector[1] + matrix[2][2] * vector[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::{Matrix3, apply};

    #[test]
    fn identity_leaves_a_vector_alone() {
        let identity: Matrix3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(apply(&identity, [1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn rows_are_dotted_with_the_vector() {
        let matrix: Matrix3 = [[1.0, 2.0, 3.0], [0.0, 1.0, 0.0], [0.0, 0.0, 2.0]];
        assert_eq!(apply(&matrix, [1.0, 1.0, 1.0]), [6.0, 1.0, 2.0]);
    }
}
