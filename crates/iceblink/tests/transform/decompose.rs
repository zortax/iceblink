//! Matrix decomposition.

use core::f32::consts::FRAC_PI_3;

use iceblink::geom::{Decomposed, Matrix4};
use proptest::prelude::*;

/// The largest entrywise difference between two matrices.
fn difference(left: &Matrix4, right: &Matrix4) -> f32 {
    let mut worst = 0.0_f32;
    for column in 0..4 {
        for row in 0..4 {
            worst = worst.max((left.get(row, column) - right.get(row, column)).abs());
        }
    }
    worst
}

/// The largest absolute entry of a matrix, which is the scale errors are relative to.
fn largest_entry(matrix: &Matrix4) -> f32 {
    matrix
        .columns
        .iter()
        .flatten()
        .fold(0.0_f32, |worst, entry| worst.max(entry.abs()))
}

/// A rotation of `radians` about a unit axis, built the long way round.
fn rotation(axis: [f32; 3], radians: f32) -> Matrix4 {
    let half = radians * 0.5;
    let sin = half.sin();
    let [x, y, z, w] = [axis[0] * sin, axis[1] * sin, axis[2] * sin, half.cos()];
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

#[test]
fn the_identity_decomposes_to_nothing() {
    let parts = Matrix4::IDENTITY.decompose().expect("decomposable");
    assert_eq!(parts, Decomposed::IDENTITY);
}

#[test]
fn a_singular_matrix_cannot_be_decomposed() {
    assert_eq!(Matrix4::scale(1.0, 0.0, 1.0).decompose(), None);
}

#[test]
fn a_reflection_shows_up_as_a_negative_scale() {
    let parts = Matrix4::scale(-1.0, 1.0, 1.0)
        .decompose()
        .expect("decomposable");
    assert!(parts.scale.iter().filter(|factor| **factor < 0.0).count() % 2 == 1);
    assert!(difference(&parts.recompose(), &Matrix4::scale(-1.0, 1.0, 1.0)) < 1e-5);
}

#[test]
fn a_rotation_survives_the_round_trip() {
    let matrix = rotation([0.0, 0.0, 1.0], FRAC_PI_3);
    let round_tripped = matrix.decompose().expect("decomposable").recompose();
    assert!(difference(&matrix, &round_tripped) < 1e-5);
}

#[test]
fn the_rotation_quaternion_has_a_non_negative_w() {
    let parts = rotation([0.0, 0.0, 1.0], 200_f32.to_radians())
        .decompose()
        .expect("decomposable");
    let expected = [0.0, 0.0, -0.984_807_8, 0.173_648_18];
    for (component, expected) in parts.rotation.into_iter().zip(expected) {
        assert!(
            (component - expected).abs() < 1e-5,
            "{:?} is not {expected:?}",
            parts.rotation
        );
    }
}

#[test]
fn perspective_survives_the_round_trip() {
    let matrix = Matrix4::translation(30.0, 10.0, -20.0).then(&Matrix4::perspective(400.0));
    let round_tripped = matrix.decompose().expect("decomposable").recompose();
    let expected = matrix.normalized().expect("normalisable");
    assert!(
        difference(&expected, &round_tripped) < 1e-4,
        "{round_tripped:?}"
    );
}

#[test]
fn a_tiny_rotation_survives_the_round_trip() {
    // A case proptest once shrank to: a rotation of about 1e-4 radians at a quarter scale.
    let axis = [0.0, 0.0, -1.0];
    let matrix = Matrix4::scale(0.25, 0.25, 0.25)
        .then(&rotation(axis, -0.000_116_704_934))
        .then(&Matrix4::perspective(200.0));
    let expected = matrix.normalized().expect("normalisable");
    let round_tripped = matrix.decompose().expect("decomposable").recompose();
    let worst = difference(&expected, &round_tripped);
    assert!(
        worst <= 1e-5 * largest_entry(&expected).max(1.0),
        "differs by {worst}"
    );
}

proptest! {
    /// Decomposing and recomposing reproduces the matrix to within 1e-5.
    #[test]
    fn decompose_and_recompose_agree(
        translation in prop::array::uniform3(-500.0_f32..500.0),
        scale in prop::array::uniform3(0.25_f32..4.0),
        angle in -3.0_f32..3.0,
        axis in prop::array::uniform3(-1.0_f32..1.0),
        depth in 200.0_f32..2000.0,
        skew_angle in -0.8_f32..0.8,
    ) {
        let axis_length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        prop_assume!(axis_length > 0.1);
        let unit = [
            axis[0] / axis_length,
            axis[1] / axis_length,
            axis[2] / axis_length,
        ];

        let mut shear = Matrix4::IDENTITY;
        shear.columns[1][0] = skew_angle.tan();

        let matrix = Matrix4::scale(scale[0], scale[1], scale[2])
            .then(&shear)
            .then(&rotation(unit, angle))
            .then(&Matrix4::translation(translation[0], translation[1], translation[2]))
            .then(&Matrix4::perspective(depth));

        let expected = matrix.normalized().expect("normalisable");
        // Normalising by a factor near zero is numerically meaningless. The viewer then sits on
        // the plane the content was pushed onto.
        prop_assume!(matrix.get(3, 3).abs() > 0.25);

        let parts = matrix.decompose().expect("decomposable");
        let round_tripped = parts.recompose();
        let worst = difference(&expected, &round_tripped);
        let magnitude = largest_entry(&expected).max(1.0);
        prop_assert!(worst <= 1e-5 * magnitude, "differs by {worst} on {expected:?}");
    }
}
