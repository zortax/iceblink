//! Blend modes.

use crate::bits::enumeration;

enumeration! {
    /// How a group is blended onto the target beneath it.
    ///
    /// The modes follow W3C Compositing and Blending Level 1 on gamma-encoded values.
    /// [`BlendMode::PLUS_LIGHTER`] adds the premultiplied channels, clamped to 1.
    pub struct BlendMode(u8) {
        /// Source-over.
        NORMAL = 0,
        /// The product of the colours.
        MULTIPLY = 1,
        /// The inverse of the product of the inverted colours.
        SCREEN = 2,
        /// Multiply or screen, depending on the backdrop.
        OVERLAY = 3,
        /// The darker of the colours.
        DARKEN = 4,
        /// The lighter of the colours.
        LIGHTEN = 5,
        /// The backdrop brightened to reflect the source.
        COLOR_DODGE = 6,
        /// The backdrop darkened to reflect the source.
        COLOR_BURN = 7,
        /// Multiply or screen, depending on the source.
        HARD_LIGHT = 8,
        /// Darken or lighten, depending on the source.
        SOFT_LIGHT = 9,
        /// The absolute difference of the colours.
        DIFFERENCE = 10,
        /// A lower-contrast difference.
        EXCLUSION = 11,
        /// The source's hue with the backdrop's saturation and luminosity.
        HUE = 12,
        /// The source's saturation with the backdrop's hue and luminosity.
        SATURATION = 13,
        /// The source's hue and saturation with the backdrop's luminosity.
        COLOR = 14,
        /// The source's luminosity with the backdrop's hue and saturation.
        LUMINOSITY = 15,
        /// The sum of the colours.
        PLUS_LIGHTER = 16,
    }
}
