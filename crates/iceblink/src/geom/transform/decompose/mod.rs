//! Splitting a 4x4 transform into parts and multiplying them back together.

mod quaternion;
mod vector;

use crate::geom::transform::Matrix4;
use crate::geom::transform::decompose::quaternion::{quaternion_of, rotation_matrix};
use crate::geom::transform::decompose::vector::{combine, cross, dot, length, unit_vector};

/// A [`Matrix4`] split into translation, scale, skew, perspective and a rotation quaternion.
///
/// Interpolating the parts of two matrices and recomposing them gives a transform between the two.
///
/// ```
/// use iceblink::geom::Matrix4;
///
/// let matrix = Matrix4::translation(10.0, 20.0, 0.0).then(&Matrix4::scale(2.0, 3.0, 1.0));
/// let parts = matrix.decompose().expect("decomposable");
/// assert_eq!(parts.scale, [2.0, 3.0, 1.0]);
/// assert_eq!(parts.translation, [20.0, 60.0, 0.0]);
/// ```
///
/// [`Decomposed::recompose`] inverts [`Matrix4::decompose`] up to floating-point rounding, and
/// returns the matrix in its [normalised](Matrix4::normalized) form.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decomposed {
    /// The translation along x, y and z.
    pub translation: [f32; 3],
    /// The scale factors along x, y and z. A negative factor is a reflection.
    pub scale: [f32; 3],
    /// The shear factors, in the order xy, xz, yz.
    pub skew: [f32; 3],
    /// The perspective row as `[x, y, z, w]`, which is `[0, 0, 0, 1]` without perspective.
    pub perspective: [f32; 4],
    /// The rotation as a unit quaternion, in the order `[x, y, z, w]`.
    pub rotation: [f32; 4],
}

impl Decomposed {
    /// The decomposition of the identity matrix.
    pub const IDENTITY: Self = Self {
        translation: [0.0; 3],
        scale: [1.0; 3],
        skew: [0.0; 3],
        perspective: [0.0, 0.0, 0.0, 1.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
    };

    /// Splits `matrix` into its parts, or returns `None` if it cannot be decomposed.
    pub(crate) fn of(matrix: &Matrix4) -> Option<Self> {
        // Normalising divides out an overall factor, which leaves the transform unchanged.
        let m = matrix.normalized()?.columns;

        let perspective = perspective_of(&m)?;
        let translation = [m[3][0], m[3][1], m[3][2]];
        let linear = split_basis([
            [m[0][0], m[0][1], m[0][2]],
            [m[1][0], m[1][1], m[1][2]],
            [m[2][0], m[2][1], m[2][2]],
        ])?;
        let rotation = quaternion_of(&linear.rotation);

        Some(Self {
            translation,
            scale: linear.scale,
            skew: linear.skew,
            perspective,
            rotation,
        })
    }

    /// Multiplies the parts back into a single matrix.
    ///
    /// The result is the [normalised](Matrix4::normalized) form of the decomposed matrix.
    ///
    /// ```
    /// use iceblink::geom::Matrix4;
    ///
    /// let matrix = Matrix4::translation(4.0, 0.0, 0.0).then(&Matrix4::scale(1.0, 2.0, 3.0));
    /// let round_tripped = matrix.decompose().expect("decomposable").recompose();
    /// for column in 0..4 {
    ///     for row in 0..4 {
    ///         let difference = matrix.get(row, column) - round_tripped.get(row, column);
    ///         assert!(difference.abs() < 1e-5);
    ///     }
    /// }
    /// ```
    pub fn recompose(&self) -> Matrix4 {
        let mut matrix = Matrix4::IDENTITY;
        for column in 0..4 {
            matrix.columns[column][3] = self.perspective[column];
        }

        let mut translation = Matrix4::IDENTITY;
        translation.columns[3] = [
            self.translation[0],
            self.translation[1],
            self.translation[2],
            1.0,
        ];
        matrix = multiply(&matrix, &translation);
        matrix = multiply(&matrix, &rotation_matrix(self.rotation));

        let mut shear = Matrix4::IDENTITY;
        shear.columns[1][0] = self.skew[0];
        shear.columns[2][0] = self.skew[1];
        shear.columns[2][1] = self.skew[2];
        matrix = multiply(&matrix, &shear);

        for (column, scale) in matrix.columns.iter_mut().zip(self.scale) {
            for value in column {
                *value *= scale;
            }
        }
        matrix
    }
}

impl Default for Decomposed {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Returns the perspective row of the normalised columns `m`, or `None` if the matrix without
/// that row is singular.
fn perspective_of(m: &[[f32; 4]; 4]) -> Option<[f32; 4]> {
    let mut without_perspective = *m;
    for column in without_perspective.iter_mut().take(3) {
        column[3] = 0.0;
    }
    without_perspective[3][3] = 1.0;
    let rest = Matrix4::from_columns(without_perspective);
    let inverse_rest = rest.invert()?;

    if m[0][3] != 0.0 || m[1][3] != 0.0 || m[2][3] != 0.0 {
        let right_hand_side = [m[0][3], m[1][3], m[2][3], m[3][3]];
        Some(inverse_rest.transpose().transform_vector4(right_hand_side))
    } else {
        Some([0.0, 0.0, 0.0, 1.0])
    }
}

/// The linear part of a matrix, split into scale, shear and rotation.
struct Linear {
    scale: [f32; 3],
    skew: [f32; 3],
    /// The rotation, as a right-handed orthonormal basis.
    rotation: [[f32; 3]; 3],
}

/// Splits three basis vectors into scale, shear and rotation.
///
/// Returns `None` if a vector has no length once the earlier ones are taken out of it.
fn split_basis(mut basis: [[f32; 3]; 3]) -> Option<Linear> {
    let mut scale = [0.0_f32; 3];
    let mut skew = [0.0_f32; 3];

    scale[0] = length(basis[0]);
    basis[0] = unit_vector(basis[0], scale[0])?;

    skew[0] = dot(basis[0], basis[1]);
    basis[1] = combine(basis[1], basis[0], -skew[0]);
    scale[1] = length(basis[1]);
    basis[1] = unit_vector(basis[1], scale[1])?;
    skew[0] /= scale[1];

    skew[1] = dot(basis[0], basis[2]);
    basis[2] = combine(basis[2], basis[0], -skew[1]);
    skew[2] = dot(basis[1], basis[2]);
    basis[2] = combine(basis[2], basis[1], -skew[2]);
    scale[2] = length(basis[2]);
    basis[2] = unit_vector(basis[2], scale[2])?;
    skew[1] /= scale[2];
    skew[2] /= scale[2];

    // A left-handed basis is a reflection. It moves into the scale so a pure rotation is left.
    if dot(basis[0], cross(basis[1], basis[2])) < 0.0 {
        for axis in 0..3 {
            scale[axis] = -scale[axis];
            basis[axis] = [-basis[axis][0], -basis[axis][1], -basis[axis][2]];
        }
    }

    Some(Linear {
        scale,
        skew,
        rotation: basis,
    })
}

/// Returns the matrix product `left * right`.
fn multiply(left: &Matrix4, right: &Matrix4) -> Matrix4 {
    right.then(left)
}
