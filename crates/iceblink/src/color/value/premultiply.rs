//! Premultiplied sRGB for renderers, and premultiplied linear light for interpolation.
//!
//! Everything a renderer holds, from surfaces to intermediate targets and atlas tiles, is
//! premultiplied, gamma-encoded sRGB, which [`Color::to_premultiplied_srgb`] produces.
//! [`Color::to_premultiplied_linear`] serves interpolation in linear light, and its result converts
//! only back into a [`Color`].

use crate::color::space::ColorSpace;
use crate::color::value::Color;

impl Color {
    /// Converts the colour to premultiplied, gamma-encoded sRGB as `[red, green, blue, alpha]`.
    ///
    /// Every channel and the alpha are clamped to `0..=1` before the channels are multiplied by
    /// the alpha. An infinite channel becomes the end of the range it ran off, and a NaN channel
    /// becomes zero.
    ///
    /// ```
    /// use iceblink::color::Color;
    ///
    /// let half_red = Color::srgb(1.0, 0.0, 0.0, 0.5);
    /// assert_eq!(half_red.to_premultiplied_srgb(), [0.5, 0.0, 0.0, 0.5]);
    ///
    /// // Transparent colours contribute nothing, whatever their channels say.
    /// assert_eq!(Color::WHITE.with_alpha(0.0).to_premultiplied_srgb(), [0.0; 4]);
    /// ```
    pub fn to_premultiplied_srgb(self) -> [f32; 4] {
        // Renderers blend gamma-encoded values, as CSS specifies filters, gradients and
        // compositing, and as every browser blends.
        let srgb = self.to_space(ColorSpace::Srgb);
        let alpha = unit(srgb.alpha);
        let [red, green, blue] = srgb.components.map(unit);
        [red * alpha, green * alpha, blue * alpha, alpha]
    }

    /// Converts the colour to premultiplied linear-light sRGB, for interpolation in linear light.
    ///
    /// ```
    /// use iceblink::color::{Color, ColorSpace};
    ///
    /// let grey = Color::srgb(0.5, 0.5, 0.5, 1.0);
    /// let linear = grey.to_premultiplied_linear().to_color();
    ///
    /// assert_eq!(linear.space(), ColorSpace::SrgbLinear);
    /// // Half-way up the encoded ramp is about a fifth of the light.
    /// assert!((linear.components()[0] - 0.2140).abs() < 1e-3);
    /// ```
    pub fn to_premultiplied_linear(self) -> PremultipliedLinear {
        let linear = self.to_space(ColorSpace::SrgbLinear);
        let alpha = linear.alpha;
        let [red, green, blue] = linear.components;
        PremultipliedLinear {
            rgba: [red * alpha, green * alpha, blue * alpha, alpha],
        }
    }
}

/// Clamps a channel to `0..=1`, mapping NaN to zero.
fn unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// A colour in premultiplied linear-light sRGB.
///
/// The four values cannot be read out. [`PremultipliedLinear::to_color`] converts the value back
/// into a [`Color`].
///
/// ```
/// use iceblink::color::Color;
///
/// let black = Color::BLACK.to_premultiplied_linear();
/// let white = Color::WHITE.to_premultiplied_linear();
/// let middle = black.lerp(white, 0.5).to_color();
///
/// // Half the *light*, which is a good deal brighter than half the encoded value.
/// let red = middle.to_space(iceblink::color::ColorSpace::Srgb).components()[0];
/// assert!((red - 0.7354).abs() < 1e-3);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PremultipliedLinear {
    rgba: [f32; 4],
}

impl PremultipliedLinear {
    /// Interpolates linearly towards `other`, where a `t` of zero gives `self` and one gives
    /// `other`.
    ///
    /// The midpoint between opaque red and transparent black is red at half alpha.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let mut rgba = [0.0; 4];
        for (channel, value) in rgba.iter_mut().enumerate() {
            *value = self.rgba[channel] + (other.rgba[channel] - self.rgba[channel]) * t;
        }
        Self { rgba }
    }

    /// Converts the value into a [`ColorSpace::SrgbLinear`] colour with straight alpha.
    ///
    /// A fully transparent value keeps its channels as they are.
    pub fn to_color(self) -> Color {
        let [red, green, blue, alpha] = self.rgba;
        let components = if alpha == 0.0 {
            [red, green, blue]
        } else {
            [red / alpha, green / alpha, blue / alpha]
        };
        Color::new(ColorSpace::SrgbLinear, components, alpha)
    }
}
