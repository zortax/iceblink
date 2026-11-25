//! 4x4 matrices.

use iceblink::geom::Matrix4;

fn close(left: [f32; 3], right: [f32; 3]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(a, b)| (a - b).abs() < 1e-5)
}

#[test]
fn rows_and_columns_are_two_views_of_one_matrix() {
    let matrix = Matrix4::from_rows([
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
        [13.0, 14.0, 15.0, 16.0],
    ]);
    assert_eq!(matrix.get(0, 3), 4.0);
    assert_eq!(matrix.columns[3], [4.0, 8.0, 12.0, 16.0]);
    assert_eq!(matrix.transpose().get(3, 0), 4.0);
}

#[test]
fn composition_applies_the_left_matrix_first() {
    let compound = Matrix4::translation(1.0, 0.0, 0.0).then(&Matrix4::scale(10.0, 1.0, 1.0));
    assert!(close(
        compound.transform_point(0.0, 0.0, 0.0),
        [10.0, 0.0, 0.0]
    ));
}

#[test]
fn the_identity_is_two_dimensional_and_perspective_is_not() {
    assert!(Matrix4::IDENTITY.is_2d());
    assert!(Matrix4::translation(1.0, 2.0, 0.0).is_2d());
    assert!(!Matrix4::translation(0.0, 0.0, 1.0).is_2d());
    assert!(!Matrix4::perspective(500.0).is_2d());
}

#[test]
fn a_matrix_and_its_inverse_cancel() {
    let matrix = Matrix4::translation(3.0, -4.0, 5.0)
        .then(&Matrix4::scale(2.0, 4.0, 8.0))
        .then(&Matrix4::perspective(400.0));
    let inverse = matrix.invert().expect("invertible");
    let identity = matrix.then(&inverse);
    for column in 0..4 {
        for row in 0..4 {
            let expected = Matrix4::IDENTITY.get(row, column);
            let found = identity.get(row, column);
            assert!((found - expected).abs() < 1e-5, "{identity:?}");
        }
    }
}

#[test]
fn a_singular_matrix_has_no_inverse() {
    assert_eq!(Matrix4::scale(1.0, 0.0, 1.0).invert(), None);
}

#[test]
fn a_non_positive_perspective_does_nothing() {
    assert_eq!(Matrix4::perspective(0.0), Matrix4::IDENTITY);
    assert_eq!(Matrix4::perspective(-10.0), Matrix4::IDENTITY);
}
