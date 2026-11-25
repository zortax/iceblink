//! Colours as three channels and an alpha in a colour space.

pub mod premultiply;

pub use crate::color::value::premultiply::PremultipliedLinear;

use crate::color::convert::convert;
use crate::color::space::ColorSpace;

/// A colour: three channel values and an alpha in a colour space.
///
/// The [space documentation](crate::color::space) lists what the channels mean in each space.
/// Channel values are never clamped to their nominal ranges. [`Color::to_premultiplied_srgb`]
/// converts a colour into renderer input.
///
/// ```
/// use iceblink::color::{Color, ColorSpace};
///
/// let red = Color::srgb_u8(255, 0, 0, 255);
/// let wide = red.to_space(ColorSpace::DisplayP3);
///
/// assert_eq!(wide.space(), ColorSpace::DisplayP3);
/// // sRGB red is inside the Display P3 gamut, so it needs less of that space's red primary.
/// assert!(wide.components()[0] < 0.95);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    components: [f32; 3],
    alpha: f32,
    space: ColorSpace,
}

impl Color {
    /// Fully transparent black, in sRGB.
    pub const TRANSPARENT: Self = Self::srgb(0.0, 0.0, 0.0, 0.0);

    /// Opaque black, in sRGB.
    pub const BLACK: Self = Self::srgb(0.0, 0.0, 0.0, 1.0);

    /// Opaque white, in sRGB.
    pub const WHITE: Self = Self::srgb(1.0, 1.0, 1.0, 1.0);

    /// Creates a colour from its space, three channel values and an alpha.
    ///
    /// ```
    /// use iceblink::color::{Color, ColorSpace};
    ///
    /// let plum = Color::new(ColorSpace::Oklch, [0.7, 0.1, 320.0], 1.0);
    /// assert_eq!(plum.components()[2], 320.0);
    /// ```
    pub const fn new(space: ColorSpace, components: [f32; 3], alpha: f32) -> Self {
        Self {
            components,
            alpha,
            space,
        }
    }

    /// Creates a gamma-encoded sRGB colour from four fractions.
    ///
    /// ```
    /// use iceblink::color::Color;
    ///
    /// let half_red = Color::srgb(1.0, 0.0, 0.0, 0.5);
    /// assert_eq!(half_red.alpha(), 0.5);
    /// ```
    pub const fn srgb(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self::new(ColorSpace::Srgb, [red, green, blue], alpha)
    }

    /// Creates a gamma-encoded sRGB colour from four bytes, each scaled by `1 / 255`.
    ///
    /// ```
    /// use iceblink::color::Color;
    ///
    /// assert_eq!(Color::srgb_u8(255, 255, 255, 255), Color::WHITE);
    /// ```
    pub fn srgb_u8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        let scale = |value: u8| f32::from(value) / 255.0;
        Self::srgb(scale(red), scale(green), scale(blue), scale(alpha))
    }

    /// Returns the three channel values in the colour's own [space](Color::space).
    pub const fn components(self) -> [f32; 3] {
        self.components
    }

    /// Returns the alpha, where zero is fully transparent and one fully opaque.
    pub const fn alpha(self) -> f32 {
        self.alpha
    }

    /// Returns the space [`Color::components`] are expressed in.
    pub const fn space(self) -> ColorSpace {
        self.space
    }

    /// Returns the same colour with `alpha`.
    ///
    /// ```
    /// use iceblink::color::Color;
    ///
    /// assert_eq!(Color::BLACK.with_alpha(0.0).alpha(), 0.0);
    /// ```
    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }

    /// Reports whether the alpha is one or more.
    pub fn is_opaque(self) -> bool {
        self.alpha >= 1.0
    }

    /// Reports whether the alpha is zero or less.
    pub fn is_transparent(self) -> bool {
        self.alpha <= 0.0
    }

    /// Converts the colour to `space`.
    ///
    /// Nothing is clamped, so a colour outside the gamut of `space` has channels outside their
    /// nominal ranges, and converting back returns the original colour up to rounding. A hue
    /// channel in the result lies in `0..360`, and the hue of a grey is zero.
    ///
    /// ```
    /// use iceblink::color::{Color, ColorSpace};
    ///
    /// let red = Color::srgb(1.0, 0.0, 0.0, 1.0);
    /// let oklch = red.to_space(ColorSpace::Oklch);
    /// let back = oklch.to_space(ColorSpace::Srgb);
    ///
    /// assert!((back.components()[0] - 1.0).abs() < 1e-4);
    /// assert!(back.components()[1].abs() < 1e-4);
    /// ```
    pub fn to_space(self, space: ColorSpace) -> Self {
        Self {
            components: convert(self.components, self.space, space),
            alpha: self.alpha,
            space,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Color;
    use crate::color::space::ColorSpace;

    #[test]
    fn bytes_scale_to_fractions() {
        let grey = Color::srgb_u8(128, 128, 128, 51);
        assert!((grey.components()[0] - 128.0 / 255.0).abs() < 1e-6);
        assert!((grey.alpha() - 0.2).abs() < 1e-3);
    }

    #[test]
    fn converting_to_the_same_space_changes_nothing() {
        let color = Color::new(ColorSpace::Lch, [50.0, 30.0, 200.0], 0.5);
        assert_eq!(color.to_space(ColorSpace::Lch), color);
    }

    #[test]
    fn conversion_leaves_alpha_alone() {
        let color = Color::srgb(0.2, 0.4, 0.6, 0.25);
        for space in ColorSpace::ALL {
            assert_eq!(color.to_space(space).alpha(), 0.25, "{space:?}");
        }
    }

    #[test]
    fn opacity_is_reported_at_the_extremes() {
        assert!(Color::WHITE.is_opaque());
        assert!(!Color::WHITE.is_transparent());
        assert!(Color::TRANSPARENT.is_transparent());
        assert!(!Color::TRANSPARENT.is_opaque());
    }
}
