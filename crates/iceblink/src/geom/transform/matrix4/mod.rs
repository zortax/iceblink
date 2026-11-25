//! The 4x4 transform matrix.

mod inverse;
mod plane;

use core::ops::Mul;

use crate::geom::transform::Decomposed;

/// A 4x4 transform matrix in column-major order.
///
/// `columns[c][r]` is the entry in column `c` and row `r`, the layout of a shader `mat4x4<f32>`.
/// The matrix maps a column vector `v` to `M * v`, so the translation is in the last column and
/// the perspective terms are in the last row. A point `(x, y)` of the plane maps to
/// `(X / W, Y / W)`, where `[X, Y, Z, W] = M * [x, y, 0, 1]`.
///
/// ```
/// use iceblink::geom::Matrix4;
///
/// let matrix = Matrix4::translation(1.0, 2.0, 3.0);
/// assert_eq!(matrix.columns[3], [1.0, 2.0, 3.0, 1.0]);
/// assert_eq!(matrix.transform_point(0.0, 0.0, 0.0), [1.0, 2.0, 3.0]);
/// ```
///
/// [`Matrix4::decompose`] splits a matrix into translation, scale, skew, perspective and
/// rotation.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4 {
    /// The four columns, each holding four rows.
    pub columns: [[f32; 4]; 4],
}

impl Matrix4 {
    /// The identity matrix.
    pub const IDENTITY: Self = Self::from_columns([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    /// Creates a matrix from its columns.
    pub const fn from_columns(columns: [[f32; 4]; 4]) -> Self {
        Self { columns }
    }

    /// Creates a matrix from its rows.
    pub const fn from_rows(rows: [[f32; 4]; 4]) -> Self {
        Self::from_columns([
            [rows[0][0], rows[1][0], rows[2][0], rows[3][0]],
            [rows[0][1], rows[1][1], rows[2][1], rows[3][1]],
            [rows[0][2], rows[1][2], rows[2][2], rows[3][2]],
            [rows[0][3], rows[1][3], rows[2][3], rows[3][3]],
        ])
    }

    /// Creates a translation.
    pub const fn translation(x: f32, y: f32, z: f32) -> Self {
        let mut matrix = Self::IDENTITY;
        matrix.columns[3] = [x, y, z, 1.0];
        matrix
    }

    /// Creates a scale about the origin.
    pub const fn scale(x: f32, y: f32, z: f32) -> Self {
        Self::from_columns([
            [x, 0.0, 0.0, 0.0],
            [0.0, y, 0.0, 0.0],
            [0.0, 0.0, z, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    /// Creates the perspective matrix for a viewer `depth` units in front of the z = 0 plane.
    ///
    /// A depth of zero or less gives the identity.
    pub fn perspective(depth: f32) -> Self {
        let mut matrix = Self::IDENTITY;
        if depth > 0.0 {
            matrix.columns[2][3] = -depth.recip();
        }
        matrix
    }

    /// Divides every entry by `get(3, 3)`, so that entry becomes one.
    ///
    /// Matrices that differ by an overall factor describe the same transform, and their normalised
    /// forms are equal.
    ///
    /// Returns `None` if `get(3, 3)` is zero or not finite.
    pub fn normalized(&self) -> Option<Self> {
        let homogeneous = self.get(3, 3);
        if homogeneous == 0.0 || !homogeneous.is_finite() {
            return None;
        }

        let inverse = homogeneous.recip();
        let mut result = *self;
        for column in &mut result.columns {
            for entry in column.iter_mut() {
                *entry *= inverse;
            }
        }
        Some(result)
    }

    /// Returns the entry in row `row` and column `column`.
    pub const fn get(&self, row: usize, column: usize) -> f32 {
        self.columns[column][row]
    }

    /// Returns the matrix with rows and columns exchanged.
    pub fn transpose(&self) -> Self {
        let mut result = *self;
        for row in 0..4 {
            for column in 0..4 {
                result.columns[column][row] = self.columns[row][column];
            }
        }
        result
    }

    /// Returns this transform followed by `next`, which is the matrix product `next * self`.
    pub fn then(&self, next: &Self) -> Self {
        let mut result = Self::from_columns([[0.0; 4]; 4]);
        for column in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0;
                for index in 0..4 {
                    sum += next.columns[index][row] * self.columns[column][index];
                }
                result.columns[column][row] = sum;
            }
        }
        result
    }

    /// Applies the transform to a point and divides by the resulting `w`.
    ///
    /// If `w` is zero, the coordinates are returned undivided.
    pub fn transform_point(&self, x: f32, y: f32, z: f32) -> [f32; 3] {
        let [out_x, out_y, out_z, w] = self.transform_vector4([x, y, z, 1.0]);
        if w == 0.0 || w == 1.0 {
            [out_x, out_y, out_z]
        } else {
            let inverse = w.recip();
            [out_x * inverse, out_y * inverse, out_z * inverse]
        }
    }

    /// Applies the transform to a homogeneous vector without dividing by `w`.
    pub fn transform_vector4(&self, vector: [f32; 4]) -> [f32; 4] {
        let mut result = [0.0; 4];
        for (row, entry) in result.iter_mut().enumerate() {
            *entry = self
                .columns
                .iter()
                .zip(vector.iter())
                .map(|(column, component)| column[row] * component)
                .sum();
        }
        result
    }

    /// Splits the matrix into translation, scale, skew, perspective and rotation.
    ///
    /// The rotation quaternion's `w` is not negative, as CSS Transforms Level 2 decomposes it.
    ///
    /// Returns `None` if the matrix has no [normalised](Matrix4::normalized) form, or if it
    /// collapses space along some axis.
    pub fn decompose(&self) -> Option<Decomposed> {
        Decomposed::of(self)
    }
}

impl Default for Matrix4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Matrix4 {
    type Output = Self;

    /// Returns the matrix product, so `self` is applied after `other`.
    fn mul(self, other: Self) -> Self {
        other.then(&self)
    }
}
