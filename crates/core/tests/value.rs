//! Keyframe selection and interpolation.

use iceblink::smallvec::smallvec;
use iceblink::{Easing, Keyframe, Keyframes, Matrix4};
use iceblink_core::motion::value::{Value, interpolate_matrix, value_at};

fn frame<T>(offset: f32, value: T, easing: Easing) -> Keyframe<T> {
    Keyframe {
        offset,
        value,
        easing,
    }
}

#[test]
fn the_progress_selects_the_pair_around_it_and_its_easing() {
    let keyframes = Keyframes::Scalar(smallvec![
        frame(0.0, 0.0, Easing::Linear),
        frame(
            0.5,
            10.0,
            Easing::Steps {
                count: 1,
                jump: iceblink::StepJump::End,
            }
        ),
        frame(1.0, 20.0, Easing::Linear),
    ]);
    assert_eq!(value_at(&keyframes, 0.25, 1.0), Some(Value::Scalar(5.0)));
    // The second pair steps at its end.
    assert_eq!(value_at(&keyframes, 0.75, 1.0), Some(Value::Scalar(10.0)));
    assert_eq!(value_at(&keyframes, 1.0, 1.0), Some(Value::Scalar(20.0)));
    // Outside `[0, 1]` the first and last pairs extrapolate.
    assert_eq!(value_at(&keyframes, -0.1, 1.0), Some(Value::Scalar(-2.0)));
}

#[test]
fn vectors_interpolate_per_component() {
    let keyframes = Keyframes::Vector(smallvec![
        frame(0.0, [0.0, 100.0], Easing::Linear),
        frame(1.0, [40.0, 0.0], Easing::Linear),
    ]);
    assert_eq!(
        value_at(&keyframes, 0.25, 1.0),
        Some(Value::Vector([10.0, 75.0]))
    );
}

#[test]
fn matrices_interpolate_by_decomposition() {
    let from = Matrix4::translation(0.0, 0.0, 0.0);
    let turned =
        rotation(core::f32::consts::FRAC_PI_2).then(&Matrix4::translation(100.0, 0.0, 0.0));
    let half = interpolate_matrix(&from, &turned, 0.5);
    let expected =
        rotation(core::f32::consts::FRAC_PI_4).then(&Matrix4::translation(50.0, 0.0, 0.0));
    for column in 0..4 {
        for row in 0..4 {
            let error = half.get(row, column) - expected.get(row, column);
            assert!(error.abs() < 1e-5, "{half:?} is not {expected:?}");
        }
    }
    assert!(half.is_2d());
}

#[test]
fn a_rotation_past_a_half_turn_interpolates_the_short_way() {
    let half = interpolate_matrix(&Matrix4::IDENTITY, &rotation(200_f32.to_radians()), 0.5);
    let expected = rotation((-80_f32).to_radians());
    for column in 0..4 {
        for row in 0..4 {
            let error = half.get(row, column) - expected.get(row, column);
            assert!(error.abs() < 1e-5, "{half:?} is not {expected:?}");
        }
    }
}

#[test]
fn a_matrix_that_cannot_be_decomposed_switches_at_one_half() {
    let from = Matrix4::IDENTITY;
    let flat = Matrix4::scale(0.0, 1.0, 1.0);
    assert_eq!(interpolate_matrix(&from, &flat, 0.49), from);
    assert_eq!(interpolate_matrix(&from, &flat, 0.5), flat);
}

/// Returns a turn by `radians` about the z axis.
fn rotation(radians: f32) -> Matrix4 {
    let (sin, cos) = radians.sin_cos();
    Matrix4::from_columns([
        [cos, sin, 0.0, 0.0],
        [-sin, cos, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}
