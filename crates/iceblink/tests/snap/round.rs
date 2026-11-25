//! Length rounding.

use iceblink::geom::DevicePx;
use iceblink::geom::snap::{ceil_length, floor_length, round_half_toward_zero, snap_length};
use proptest::prelude::*;

#[test]
fn halves_break_toward_zero() {
    assert_eq!(round_half_toward_zero(0.5), 0.0);
    assert_eq!(round_half_toward_zero(-0.5), -0.0);
    assert_eq!(round_half_toward_zero(1.5), 1.0);
    assert_eq!(round_half_toward_zero(-1.5), -1.0);
}

#[test]
fn anything_past_a_half_rounds_away() {
    assert_eq!(round_half_toward_zero(1.500_001), 2.0);
    assert_eq!(round_half_toward_zero(-1.500_001), -2.0);
}

#[test]
fn a_coordinate_too_large_to_have_a_fraction_does_not_move() {
    // Past 2^23 consecutive floats are a whole pixel apart, so every value is already on the
    // grid and rounding must be the identity rather than a one-pixel jump.
    for value in [8_388_609.0_f32, -8_388_609.0, 1e30, -1e30] {
        assert_eq!(round_half_toward_zero(value), value);
        assert_eq!(snap_length(DevicePx(value)), DevicePx(value));
    }
}

#[test]
fn non_finite_lengths_pass_through() {
    assert_eq!(round_half_toward_zero(f32::INFINITY), f32::INFINITY);
    assert_eq!(round_half_toward_zero(f32::NEG_INFINITY), f32::NEG_INFINITY);
    assert!(round_half_toward_zero(f32::NAN).is_nan());
}

proptest! {
    /// Every rounding rule lands on the grid and stays there.
    #[test]
    fn rounding_is_idempotent(value in -1e9_f32..1e9) {
        let length = DevicePx(value);
        for rounded in [snap_length(length), floor_length(length), ceil_length(length)] {
            prop_assert!(rounded.is_grid_aligned());
            prop_assert_eq!(snap_length(rounded), rounded);
            prop_assert_eq!(floor_length(rounded), rounded);
            prop_assert_eq!(ceil_length(rounded), rounded);
        }
    }

    /// Flooring never moves up and ceiling never moves down.
    #[test]
    fn covering_brackets_the_value(value in -1e9_f32..1e9) {
        let length = DevicePx(value);
        prop_assert!(floor_length(length) <= length);
        prop_assert!(ceil_length(length) >= length);
    }
}
