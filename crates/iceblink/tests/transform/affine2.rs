//! Two-dimensional affine transforms.

use core::f32::consts::FRAC_PI_2;

use iceblink::geom::{Affine2, Logical, LogicalPx, Point};

fn point(x: f32, y: f32) -> Point<LogicalPx, Logical> {
    Point::new(LogicalPx(x), LogicalPx(y))
}

fn close(left: Point<LogicalPx, Logical>, right: Point<LogicalPx, Logical>) -> bool {
    (left.x.0 - right.x.0).abs() < 1e-4 && (left.y.0 - right.y.0).abs() < 1e-4
}

#[test]
fn a_quarter_turn_takes_the_x_axis_to_the_y_axis() {
    let turned = Affine2::rotation(FRAC_PI_2).transform_point(point(1.0, 0.0));
    assert!(close(turned, point(0.0, 1.0)));
}

#[test]
fn composition_applies_the_left_transform_first() {
    let compound = Affine2::translation(1.0, 0.0).then(Affine2::scale(10.0, 10.0));
    assert!(close(
        compound.transform_point(point(0.0, 0.0)),
        point(10.0, 0.0)
    ));
}

#[test]
fn multiplication_is_composition_the_other_way_round() {
    let left = Affine2::translation(1.0, 2.0);
    let right = Affine2::scale(3.0, 4.0);
    assert_eq!(left * right, right.then(left));
}

#[test]
fn a_transform_and_its_inverse_cancel() {
    let transform = Affine2::translation(5.0, -2.0)
        .then(Affine2::rotation(0.6))
        .then(Affine2::scale(2.0, 3.0));
    let inverse = transform.invert().expect("invertible");
    let round_tripped = inverse.transform_point(transform.transform_point(point(7.0, -9.0)));
    assert!(close(round_tripped, point(7.0, -9.0)));
}

#[test]
fn a_collapsed_transform_has_no_inverse() {
    assert_eq!(Affine2::scale(0.0, 1.0).invert(), None);
}

#[test]
fn the_translation_lands_in_the_last_matrix4_column() {
    let matrix = Affine2::translation(3.0, 4.0).to_matrix4();
    assert_eq!(matrix.columns[3], [3.0, 4.0, 0.0, 1.0]);
}
