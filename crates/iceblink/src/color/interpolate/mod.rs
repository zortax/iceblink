//! Interpolation between two colours by the rules of CSS Color 4.
//!
//! Both colours are converted into the interpolation space and premultiplied, and the channels are
//! interpolated linearly. The result is then divided by its alpha.

pub(crate) mod channels;
pub mod hue;

pub use crate::color::interpolate::hue::HueInterpolation;

use crate::color::convert::polar::normalize_hue;
use crate::color::space::ColorSpace;
use crate::color::value::Color;

/// The space an interpolation happens in, and the way round the hue circle it travels if that
/// space has a hue.
///
/// ```
/// use iceblink::color::{ColorSpace, HueInterpolation, Interpolation};
///
/// // What `in oklab` means in a gradient.
/// let oklab = Interpolation::new(ColorSpace::Oklab);
/// assert_eq!(oklab.hue, HueInterpolation::Shorter);
///
/// // What `in hsl longer hue` means.
/// let long_way = Interpolation::new(ColorSpace::Hsl).with_hue(HueInterpolation::Longer);
/// assert_eq!(long_way.space, ColorSpace::Hsl);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Interpolation {
    /// The space the channels are interpolated in.
    pub space: ColorSpace,
    /// The arc of the hue circle to travel, ignored in spaces without a hue.
    pub hue: HueInterpolation,
}

impl Interpolation {
    /// Creates an interpolation in `space` that takes the shorter arc round the hue circle.
    pub const fn new(space: ColorSpace) -> Self {
        Self {
            space,
            hue: HueInterpolation::Shorter,
        }
    }

    /// Returns the same interpolation travelling round the hue circle by `hue`.
    pub const fn with_hue(self, hue: HueInterpolation) -> Self {
        Self { hue, ..self }
    }
}

impl Default for Interpolation {
    /// Returns interpolation in Oklab with the shorter hue arc, the default of CSS Color 4.
    fn default() -> Self {
        Self::new(ColorSpace::Oklab)
    }
}

/// Returns the colour `t` of the way from `from` to `to`, in the interpolation space.
///
/// A `t` of zero gives `from` and one gives `to`. Values outside that range extrapolate. Hues are
/// wrapped into `0..360` before the arc is chosen and after the interpolation.
///
/// ```
/// use iceblink::color::{Color, ColorSpace, Interpolation, interpolate};
///
/// let red = Color::srgb(1.0, 0.0, 0.0, 1.0);
/// let blue = Color::srgb(0.0, 0.0, 1.0, 1.0);
/// let middle = interpolate(red, blue, 0.5, Interpolation::new(ColorSpace::Srgb));
///
/// assert_eq!(middle.components(), [0.5, 0.0, 0.5]);
/// ```
///
/// Channels are interpolated premultiplied, so a ramp to transparent keeps its colour:
///
/// ```
/// use iceblink::color::{Color, ColorSpace, Interpolation, interpolate};
///
/// let blue = Color::srgb(0.0, 0.0, 1.0, 1.0);
/// let middle = interpolate(Color::TRANSPARENT, blue, 0.5, Interpolation::new(ColorSpace::Srgb));
///
/// assert_eq!(middle.components(), [0.0, 0.0, 1.0]);
/// assert_eq!(middle.alpha(), 0.5);
/// ```
pub fn interpolate(from: Color, to: Color, t: f32, interpolation: Interpolation) -> Color {
    if interpolation.space == ColorSpace::SrgbLinear {
        // `PremultipliedLinear` is this space, so the general path would repeat its arithmetic.
        return from
            .to_premultiplied_linear()
            .lerp(to.to_premultiplied_linear(), t)
            .to_color();
    }
    general(from, to, t, interpolation)
}

/// Interpolates in any space: hue fixup, premultiply, mix, then divide by the alpha.
fn general(from: Color, to: Color, t: f32, interpolation: Interpolation) -> Color {
    let space = interpolation.space;
    let from = from.to_space(space);
    let to = to.to_space(space);

    let mut start = from.components();
    let mut end = to.components();
    if let Some(index) = space.hue_index() {
        // The fixup reads the difference between the two numbers, so a hue written several turns
        // round would send the interpolation the wrong way without the wrap.
        let (fixed_from, fixed_to) = interpolation
            .hue
            .fixup(normalize_hue(start[index]), normalize_hue(end[index]));
        start[index] = fixed_from;
        end[index] = fixed_to;
    }

    let start = channels::premultiply(space, start, from.alpha());
    let end = channels::premultiply(space, end, to.alpha());

    let mix = |a: f32, b: f32| a + (b - a) * t;
    let alpha = mix(from.alpha(), to.alpha());
    let mut components = [0.0; 3];
    for (index, value) in components.iter_mut().enumerate() {
        *value = mix(start[index], end[index]);
    }

    let mut components = channels::unpremultiply(space, components, alpha);
    if let Some(index) = space.hue_index() {
        components[index] = normalize_hue(components[index]);
    }
    Color::new(space, components, alpha)
}

#[cfg(test)]
mod tests {
    use super::{Interpolation, general, interpolate};
    use crate::color::space::ColorSpace;
    use crate::color::value::Color;

    #[test]
    fn the_linear_shortcut_agrees_with_the_general_path() {
        let interpolation = Interpolation::new(ColorSpace::SrgbLinear);
        let from = Color::srgb(0.9, 0.1, 0.3, 0.25);
        let to = Color::srgb(0.1, 0.8, 0.2, 1.0);
        for step in 0i16..=10 {
            let t = f32::from(step) / 10.0;
            let shortcut = interpolate(from, to, t, interpolation);
            let generic = general(from, to, t, interpolation);
            for channel in 0..3 {
                assert!(
                    (shortcut.components()[channel] - generic.components()[channel]).abs() < 1e-6,
                    "channel {channel} at t = {t}",
                );
            }
            assert!((shortcut.alpha() - generic.alpha()).abs() < 1e-6);
        }
    }
}
