//! Interpolation between two colours.

use iceblink::color::{Color, ColorSpace, HueInterpolation, Interpolation, interpolate};

#[test]
fn the_endpoints_are_reproduced_exactly() {
    let from = Color::srgb(0.2, 0.4, 0.6, 0.8);
    let to = Color::new(ColorSpace::Oklch, [0.7, 0.15, 200.0], 0.4);
    for space in ColorSpace::ALL {
        let interpolation = Interpolation::new(space);
        let start = interpolate(from, to, 0.0, interpolation);
        let end = interpolate(from, to, 1.0, interpolation);
        for channel in 0..3 {
            let expected = from.to_space(space).components()[channel];
            assert!(
                (start.components()[channel] - expected).abs() < 1e-3,
                "{space:?} channel {channel}: {} vs {expected}",
                start.components()[channel],
            );
            let expected = to.to_space(space).components()[channel];
            assert!(
                (end.components()[channel] - expected).abs() < 1e-3,
                "{space:?} channel {channel}: {} vs {expected}",
                end.components()[channel],
            );
        }
    }
}

#[test]
fn interpolating_a_colour_with_itself_stands_still() {
    let color = Color::new(ColorSpace::Lch, [60.0, 40.0, 120.0], 0.5);
    for space in ColorSpace::ALL {
        let middle = interpolate(color, color, 0.5, Interpolation::new(space));
        let expected = color.to_space(space);
        for channel in 0..3 {
            assert!(
                (middle.components()[channel] - expected.components()[channel]).abs() < 1e-3,
                "{space:?} channel {channel}",
            );
        }
    }
}

#[test]
fn the_hue_arc_is_the_one_that_was_asked_for() {
    let from = Color::new(ColorSpace::Hsl, [20.0, 0.5, 0.5], 1.0);
    let to = Color::new(ColorSpace::Hsl, [320.0, 0.5, 0.5], 1.0);
    let hue = |method| {
        interpolate(
            from,
            to,
            0.5,
            Interpolation::new(ColorSpace::Hsl).with_hue(method),
        )
        .components()[0]
    };
    assert!((hue(HueInterpolation::Shorter) - 350.0).abs() < 1e-3);
    assert!((hue(HueInterpolation::Longer) - 170.0).abs() < 1e-3);
    assert!((hue(HueInterpolation::Increasing) - 170.0).abs() < 1e-3);
    assert!((hue(HueInterpolation::Decreasing) - 350.0).abs() < 1e-3);
}

#[test]
fn a_hue_written_several_turns_round_means_what_it_says() {
    // `hsl(720deg …)` is the same colour as `hsl(0deg …)`, so it interpolates the same way. The
    // fixup reads the difference between two numbers, and an unwrapped hue would send this one
    // two turns backwards.
    let far = Color::new(ColorSpace::Hsl, [720.0, 0.5, 0.5], 1.0);
    let near = Color::new(ColorSpace::Hsl, [0.0, 0.5, 0.5], 1.0);
    let to = Color::new(ColorSpace::Hsl, [10.0, 0.5, 0.5], 1.0);
    for method in [
        HueInterpolation::Shorter,
        HueInterpolation::Longer,
        HueInterpolation::Increasing,
        HueInterpolation::Decreasing,
    ] {
        let interpolation = Interpolation::new(ColorSpace::Hsl).with_hue(method);
        let from_far = interpolate(far, to, 0.5, interpolation).components()[0];
        let from_near = interpolate(near, to, 0.5, interpolation).components()[0];
        assert!(
            (from_far - from_near).abs() < 1e-3,
            "{method:?}: {from_far} from 720°, {from_near} from 0°",
        );
    }
}

#[test]
fn extrapolation_runs_past_the_endpoints() {
    let from = Color::srgb(0.0, 0.0, 0.0, 1.0);
    let to = Color::srgb(0.5, 0.5, 0.5, 1.0);
    let beyond = interpolate(from, to, 2.0, Interpolation::new(ColorSpace::Srgb));
    assert_eq!(beyond.components(), [1.0, 1.0, 1.0]);
}

#[test]
fn the_default_interpolation_is_oklab() {
    assert_eq!(Interpolation::default().space, ColorSpace::Oklab);
}
