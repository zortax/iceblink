//! Matrix interpolation by decomposition, as CSS Transforms Level 2 specifies.

use iceblink::Matrix4;
use iceblink::geom::Decomposed;

use crate::motion::value::lerp;

/// Returns the matrix `share` of the way from `from` to `to`.
///
/// Both matrices are decomposed into translation, scale, skew, perspective and a rotation
/// quaternion. The parts are interpolated linearly and the quaternion spherically, and the result
/// is recomposed. When either matrix cannot be decomposed, the result is `from` below a share of
/// 0.5 and `to` from there on.
///
/// Two matrices that both keep the plane as a 2D transform interpolate to one that keeps it too.
pub fn interpolate_matrix(from: &Matrix4, to: &Matrix4, share: f32) -> Matrix4 {
    let (Some(a), Some(b)) = (from.decompose(), to.decompose()) else {
        return if share < 0.5 { *from } else { *to };
    };

    let mixed = Decomposed {
        translation: mix(a.translation, b.translation, share),
        scale: mix(a.scale, b.scale, share),
        skew: mix(a.skew, b.skew, share),
        perspective: mix(a.perspective, b.perspective, share),
        rotation: slerp(a.rotation, b.rotation, share),
    };
    let mut matrix = mixed.recompose();
    if from.is_2d() && to.is_2d() {
        flatten(&mut matrix);
    }
    matrix
}

/// Interpolates every component of two arrays linearly.
fn mix<const N: usize>(from: [f32; N], to: [f32; N], share: f32) -> [f32; N] {
    core::array::from_fn(|index| lerp(from[index], to[index], share))
}

/// Interpolates two unit quaternions spherically, as CSS Transforms Level 2 specifies.
fn slerp(from: [f32; 4], to: [f32; 4], share: f32) -> [f32; 4] {
    let (from, to) = (from.map(f64::from), to.map(f64::from));
    let product = (0..4)
        .map(|index| from[index] * to[index])
        .sum::<f64>()
        .clamp(-1.0, 1.0);
    if product.abs() == 1.0 {
        return from.map(|component| component as f32);
    }

    let theta = product.acos();
    let share = f64::from(share);
    let w = (share * theta).sin() / (1.0 - product * product).sqrt();
    let keep = (share * theta).cos() - product * w;
    core::array::from_fn(|index| (from[index] * keep + to[index] * w) as f32)
}

/// Clears the rounding a 3D recomposition leaves in the rows and columns a 2D transform keeps at
/// the identity.
fn flatten(matrix: &mut Matrix4) {
    let m = &mut matrix.columns;
    m[0][2] = 0.0;
    m[0][3] = 0.0;
    m[1][2] = 0.0;
    m[1][3] = 0.0;
    m[2] = [0.0, 0.0, 1.0, 0.0];
    m[3][2] = 0.0;
    m[3][3] = 1.0;
}
