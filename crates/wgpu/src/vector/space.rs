//! Matrices as plane transforms.

use iceblink::Matrix4;
use kurbo::Affine;

/// Returns the plane transform `matrix` embeds, or `None` if it is not two-dimensional.
pub fn affine(matrix: &Matrix4) -> Option<Affine> {
    let plane = matrix.to_affine2()?;
    let coefficients = [plane.a, plane.b, plane.c, plane.d, plane.tx, plane.ty];
    Some(Affine::new(coefficients.map(f64::from)))
}
