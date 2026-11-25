//! Premultiplied colours.

use bytemuck::{Pod, Zeroable};

/// A premultiplied, gamma-encoded sRGB colour, as `[r, g, b, a]`.
///
/// Every component lies in `[0, 1]`, and `r`, `g` and `b` are at most `a`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Color(pub [f32; 4]);

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self([0.0; 4]);

    /// Premultiplies a straight-alpha, gamma-encoded sRGB colour.
    pub const fn from_straight(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([r * a, g * a, b * a, a])
    }

    /// Creates a colour from eight bits per component, each scaled by `1 / 255`.
    pub fn from_rgba8(rgba: [u8; 4]) -> Self {
        Self(rgba.map(|component| f32::from(component) / 255.0))
    }

    /// Returns the colour at eight bits per component, each computed as `(c × 255).round()` in
    /// `f32`.
    ///
    /// Halves round up, and a component outside `[0, 1]` is clamped first.
    pub fn to_rgba8(self) -> [u8; 4] {
        self.0
            .map(|component| (component.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    /// Returns the colour with each component quantised to eight bits.
    ///
    /// The result is `Color::from_rgba8(self.to_rgba8())`, so a colour made by
    /// [`Color::from_rgba8`] is returned unchanged.
    pub fn quantized(self) -> Self {
        Self::from_rgba8(self.to_rgba8())
    }

    /// Reports whether the alpha quantises to 255 with [`Color::to_rgba8`].
    pub fn is_opaque(self) -> bool {
        self.to_rgba8()[3] == u8::MAX
    }
}

impl From<crate::color::Color> for Color {
    /// Converts the colour to gamma-encoded sRGB and premultiplies it.
    fn from(color: crate::color::Color) -> Self {
        Self(color.to_premultiplied_srgb())
    }
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn eight_bit_colours_survive_quantisation() {
        for step in 0..=255_u8 {
            let rgba = [step, step / 2, 255 - step, 255];
            assert_eq!(Color::from_rgba8(rgba).to_rgba8(), rgba);
        }
    }

    #[test]
    fn halves_round_up() {
        let half = Color([191.5 / 255.0, 0.5 / 255.0, 254.5 / 255.0, 1.0]);
        assert_eq!(half.to_rgba8(), [192, 1, 255, 255]);
    }
}
