//! Mixing two colours by weight, by the `color-mix()` rules of CSS Color 4.

use crate::color::interpolate::{Interpolation, interpolate};
use crate::color::value::Color;

/// Mixes two colours by weight.
///
/// The weights are fractions, so `25%` is `0.25`. The result lies at the second weight's share of
/// their sum along the way from `first` to `second`. If the weights add up to less than one, the
/// result's alpha is also multiplied by their sum.
///
/// Returns `None` if either weight is negative or NaN, or if both are zero.
///
/// ```
/// use iceblink::color::{Color, ColorSpace, Interpolation, color_mix};
///
/// let red = Color::srgb(1.0, 0.0, 0.0, 1.0);
/// let blue = Color::srgb(0.0, 0.0, 1.0, 1.0);
/// let srgb = Interpolation::new(ColorSpace::Srgb);
///
/// let quarter = color_mix(srgb, red, 0.25, blue, 0.75).expect("weights are positive");
/// assert_eq!(quarter.components(), [0.25, 0.0, 0.75]);
///
/// let faded = color_mix(srgb, red, 0.2, blue, 0.2).expect("weights are positive");
/// assert_eq!(faded.components(), [0.5, 0.0, 0.5]);
/// assert!((faded.alpha() - 0.4).abs() < 1e-6);
/// ```
pub fn color_mix(
    interpolation: Interpolation,
    first: Color,
    first_weight: f32,
    second: Color,
    second_weight: f32,
) -> Option<Color> {
    if first_weight.is_nan() || second_weight.is_nan() {
        return None;
    }
    if first_weight < 0.0 || second_weight < 0.0 {
        return None;
    }
    let total = first_weight + second_weight;
    if total <= 0.0 {
        return None;
    }

    let mixed = interpolate(first, second, second_weight / total, interpolation);
    let alpha = if total < 1.0 {
        mixed.alpha() * total
    } else {
        mixed.alpha()
    };
    Some(mixed.with_alpha(alpha))
}

/// Mixes two colours in equal parts.
///
/// The result is that of [`color_mix`] with both weights at one half.
///
/// ```
/// use iceblink::color::{Color, ColorSpace, Interpolation, color_mix_evenly};
///
/// let grey = color_mix_evenly(
///     Interpolation::new(ColorSpace::Srgb),
///     Color::WHITE,
///     Color::BLACK,
/// );
/// assert_eq!(grey.components(), [0.5, 0.5, 0.5]);
/// ```
pub fn color_mix_evenly(interpolation: Interpolation, first: Color, second: Color) -> Color {
    interpolate(first, second, 0.5, interpolation)
}

#[cfg(test)]
mod tests {
    use super::{color_mix, color_mix_evenly};
    use crate::color::interpolate::Interpolation;
    use crate::color::space::ColorSpace;
    use crate::color::value::Color;

    /// The sRGB interpolation used where the space is beside the point.
    const SRGB: Interpolation = Interpolation::new(ColorSpace::Srgb);

    #[test]
    fn weights_are_a_ratio() {
        let red = Color::srgb(1.0, 0.0, 0.0, 1.0);
        let blue = Color::srgb(0.0, 0.0, 1.0, 1.0);
        let by_quarters = color_mix(SRGB, red, 0.25, blue, 0.75).expect("positive weights");
        let by_ratio = color_mix(SRGB, red, 1.0, blue, 3.0).expect("positive weights");
        assert_eq!(by_quarters.components(), by_ratio.components());
    }

    #[test]
    fn weights_over_one_leave_alpha_alone() {
        let mixed = color_mix(SRGB, Color::WHITE, 1.0, Color::BLACK, 3.0).expect("positive");
        assert_eq!(mixed.alpha(), 1.0);
    }

    #[test]
    fn an_empty_mix_has_no_answer() {
        assert!(color_mix(SRGB, Color::WHITE, 0.0, Color::BLACK, 0.0).is_none());
        assert!(color_mix(SRGB, Color::WHITE, -0.5, Color::BLACK, 1.0).is_none());
        assert!(color_mix(SRGB, Color::WHITE, f32::NAN, Color::BLACK, 1.0).is_none());
    }

    #[test]
    fn a_zero_weight_gives_the_other_colour_back() {
        let mixed = color_mix(SRGB, Color::WHITE, 0.0, Color::BLACK, 1.0).expect("positive");
        assert_eq!(mixed.components(), [0.0, 0.0, 0.0]);
        assert_eq!(mixed.alpha(), 1.0);
    }

    #[test]
    fn mixing_evenly_is_mixing_at_equal_weights() {
        let first = Color::new(ColorSpace::Oklch, [0.6, 0.2, 30.0], 0.5);
        let second = Color::srgb(0.1, 0.7, 0.2, 1.0);
        for space in ColorSpace::ALL {
            let interpolation = Interpolation::new(space);
            let evenly = color_mix_evenly(interpolation, first, second);
            let weighted = color_mix(interpolation, first, 0.5, second, 0.5).expect("positive");
            assert_eq!(evenly.components(), weighted.components(), "{space:?}");
            assert_eq!(evenly.alpha(), weighted.alpha(), "{space:?}");
        }
    }
}
