//! Elliptical corner radii.

use iceblink::geom::{Corners, Logical, LogicalPx, Size, Vec2};
use proptest::prelude::*;

fn size(width: f32, height: f32) -> Size<LogicalPx, Logical> {
    Size::new(LogicalPx(width), LogicalPx(height))
}

#[test]
fn an_elliptical_corner_is_not_a_circular_one() {
    let radii = Corners::uniform(Vec2::new(LogicalPx(20.0), LogicalPx(10.0)));
    assert!(!radii.is_circular());
    assert!(!radii.is_square());
}

#[test]
fn radii_that_fit_are_left_alone() {
    let radii = Corners::uniform(Vec2::new(LogicalPx(10.0), LogicalPx(5.0)));
    assert_eq!(radii.fit_within(size(100.0, 100.0)), radii);
}

#[test]
fn the_tightest_side_decides_the_shrink_factor() {
    // The vertical radii sum to 200 over a height of 100, a ratio of 0.5. The horizontal
    // ones sum to 40 over 100, which is not binding.
    let radii = Corners::uniform(Vec2::new(LogicalPx(20.0), LogicalPx(100.0)));
    let fitted = radii.fit_within(size(100.0, 100.0));
    assert_eq!(fitted.top_left, Vec2::new(LogicalPx(10.0), LogicalPx(50.0)));
    assert_eq!(
        fitted.bottom_right,
        Vec2::new(LogicalPx(10.0), LogicalPx(50.0))
    );
}

#[test]
fn shrinking_is_uniform_across_corners() {
    let radii = Corners::new(
        Vec2::splat(LogicalPx(80.0)),
        Vec2::splat(LogicalPx(20.0)),
        Vec2::splat(LogicalPx(0.0)),
        Vec2::splat(LogicalPx(0.0)),
    );
    let fitted = radii.fit_within(size(50.0, 50.0));
    // The top radii sum to 100 along a side of 50, so every radius halves.
    assert_eq!(fitted.top_left, Vec2::splat(LogicalPx(40.0)));
    assert_eq!(fitted.top_right, Vec2::splat(LogicalPx(10.0)));
}

#[test]
fn negative_radii_clamp_to_zero_before_fitting() {
    let radii = Corners::uniform(Vec2::new(LogicalPx(-5.0), LogicalPx(-5.0)));
    assert_eq!(
        radii.fit_within(size(10.0, 10.0)),
        Corners::uniform(Vec2::ZERO)
    );
}

#[test]
fn a_box_with_a_negative_extent_collapses_the_radii() {
    let radii = Corners::uniform(Vec2::splat(LogicalPx(10.0)));
    let fitted = radii.fit_within(size(-20.0, 100.0));
    assert_eq!(fitted, Corners::uniform(Vec2::ZERO));
}

#[test]
fn a_box_of_no_size_at_all_collapses_the_radii() {
    let radii = Corners::uniform(Vec2::splat(LogicalPx(10.0)));
    assert_eq!(
        radii.fit_within(size(0.0, 0.0)),
        Corners::uniform(Vec2::ZERO)
    );
}

#[test]
fn fitted_radii_no_longer_overflow_the_box() {
    let radii = Corners::uniform(Vec2::new(LogicalPx(60.0), LogicalPx(90.0)));
    let fitted = radii.fit_within(size(100.0, 100.0));
    let epsilon = 1e-3;
    assert!(fitted.top_left.x.0 + fitted.top_right.x.0 <= 100.0 + epsilon);
    assert!(fitted.top_right.y.0 + fitted.bottom_right.y.0 <= 100.0 + epsilon);
    // Refitting moves nothing more than rounding accounts for.
    let again = fitted.fit_within(size(100.0, 100.0));
    assert!((again.top_left.x.0 - fitted.top_left.x.0).abs() < epsilon);
    assert!((again.top_left.y.0 - fitted.top_left.y.0).abs() < epsilon);
}

proptest! {
    /// Whatever it is given, fitting yields radii that are non-negative and that fit.
    #[test]
    fn fitted_radii_are_non_negative_and_fit_every_side(
        radii in prop::array::uniform8(-50.0_f32..200.0),
        width in -50.0_f32..300.0,
        height in -50.0_f32..300.0,
    ) {
        let corners = Corners::new(
            Vec2::new(LogicalPx(radii[0]), LogicalPx(radii[1])),
            Vec2::new(LogicalPx(radii[2]), LogicalPx(radii[3])),
            Vec2::new(LogicalPx(radii[4]), LogicalPx(radii[5])),
            Vec2::new(LogicalPx(radii[6]), LogicalPx(radii[7])),
        );
        let fitted = corners.fit_within(size(width, height));

        for radius in fitted.into_array() {
            prop_assert!(radius.x >= LogicalPx(0.0), "negative radius in {:?}", fitted);
            prop_assert!(radius.y >= LogicalPx(0.0), "negative radius in {:?}", fitted);
        }

        let usable_width = width.max(0.0);
        let usable_height = height.max(0.0);
        let epsilon = 1e-3 * usable_width.max(usable_height).max(1.0);
        prop_assert!(
            fitted.top_left.x.0 + fitted.top_right.x.0 <= usable_width + epsilon
        );
        prop_assert!(
            fitted.bottom_left.x.0 + fitted.bottom_right.x.0 <= usable_width + epsilon
        );
        prop_assert!(
            fitted.top_left.y.0 + fitted.bottom_left.y.0 <= usable_height + epsilon
        );
        prop_assert!(
            fitted.top_right.y.0 + fitted.bottom_right.y.0 <= usable_height + epsilon
        );
    }
}
