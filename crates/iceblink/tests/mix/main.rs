//! Reference mixes worked out by hand from the CSS Color 4 mixing rules.
//!
//! Each case states the two colours, their weights, the space to mix in, and the result in that
//! space.

mod cases;

use iceblink::color::{Color, ColorSpace, Interpolation, color_mix};

use crate::cases::CASES;

/// One reference mix.
pub(crate) struct Case {
    /// What the case is about, quoted when it fails.
    pub(crate) name: &'static str,
    /// The space and hue arc to mix in.
    pub(crate) interpolation: Interpolation,
    /// The first colour and its weight.
    pub(crate) first: (Color, f32),
    /// The second colour and its weight.
    pub(crate) second: (Color, f32),
    /// The expected result, in the mixing space.
    pub(crate) expected: Color,
}

/// Returns how far each channel of `space` may be from its reference value.
///
/// The tolerance is one part in 255 of the channel's nominal range, the finest distinction an
/// eight-bit output makes.
fn tolerances(space: ColorSpace) -> [f32; 3] {
    let scale = |range: f32| range / 255.0;
    match space {
        ColorSpace::Hsl | ColorSpace::Hwb => [scale(360.0), scale(1.0), scale(1.0)],
        ColorSpace::Lab => [scale(100.0), scale(100.0), scale(100.0)],
        ColorSpace::Lch => [scale(100.0), scale(100.0), scale(360.0)],
        ColorSpace::Oklch => [scale(1.0), scale(1.0), scale(360.0)],
        _ => [scale(1.0); 3],
    }
}

/// Returns the difference between two values of one channel, treating hue as an angle.
fn difference(space: ColorSpace, index: usize, actual: f32, expected: f32) -> f32 {
    if space.hue_index() == Some(index) {
        let raw = (actual - expected).rem_euclid(360.0);
        return raw.min(360.0 - raw);
    }
    (actual - expected).abs()
}

#[test]
fn every_reference_mix_matches() {
    assert!(CASES.len() >= 40, "the reference table has shrunk");
    for case in CASES {
        let mixed = color_mix(
            case.interpolation,
            case.first.0,
            case.first.1,
            case.second.0,
            case.second.1,
        )
        .expect("every reference case has positive weights");

        assert_eq!(mixed.space(), case.expected.space(), "{}", case.name);
        let tolerances = tolerances(case.interpolation.space);
        for (channel, tolerance) in tolerances.into_iter().enumerate() {
            let actual = mixed.components()[channel];
            let expected = case.expected.components()[channel];
            let difference = difference(case.interpolation.space, channel, actual, expected);
            assert!(
                difference <= tolerance,
                "{}: channel {channel} is {actual}, expected {expected}",
                case.name,
            );
        }
        assert!(
            (mixed.alpha() - case.expected.alpha()).abs() <= 1.0 / 255.0,
            "{}: alpha is {}, expected {}",
            case.name,
            mixed.alpha(),
            case.expected.alpha(),
        );
    }
}
