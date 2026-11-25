//! The two illuminants the colour spaces are referenced to.
//!
//! A conversion between spaces of different white points adapts the values to the white point of
//! the target space.

/// A standard illuminant.
///
/// The tristimulus values are the CIE 2-degree standard observer values of CSS Color 4, scaled so
/// that `Y` is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WhitePoint {
    /// Horizon light, roughly 5000 K. The reference white of the L\*a\*b\*, LCH, ProPhoto RGB and
    /// D50 XYZ spaces.
    D50,
    /// Noon daylight, roughly 6500 K. The reference white of sRGB and the other D65 spaces.
    D65,
}

impl WhitePoint {
    /// Returns the CIE XYZ tristimulus values of this illuminant.
    ///
    /// ```
    /// use iceblink::color::WhitePoint;
    ///
    /// assert_eq!(WhitePoint::D65.tristimulus()[1], 1.0);
    /// ```
    pub const fn tristimulus(self) -> [f32; 3] {
        match self {
            Self::D50 => D50_TRISTIMULUS,
            Self::D65 => D65_TRISTIMULUS,
        }
    }
}

/// The D50 tristimulus values, `x = 0.3457`, `y = 0.3585`, normalised to `Y = 1`.
const D50_TRISTIMULUS: [f32; 3] = [0.964_295_7, 1.0, 0.825_104_6];

/// The D65 tristimulus values, `x = 0.3127`, `y = 0.3290`, normalised to `Y = 1`.
const D65_TRISTIMULUS: [f32; 3] = [0.950_455_9, 1.0, 1.089_057_8];

#[cfg(test)]
mod tests {
    use super::WhitePoint;

    /// Returns the `x` and `y` chromaticity coordinates of tristimulus values.
    fn chromaticity(tristimulus: [f32; 3]) -> (f32, f32) {
        let sum = tristimulus[0] + tristimulus[1] + tristimulus[2];
        (tristimulus[0] / sum, tristimulus[1] / sum)
    }

    #[test]
    fn tristimulus_values_match_their_chromaticities() {
        let (x, y) = chromaticity(WhitePoint::D50.tristimulus());
        assert!((x - 0.3457).abs() < 1e-4, "D50 x is {x}");
        assert!((y - 0.3585).abs() < 1e-4, "D50 y is {y}");

        let (x, y) = chromaticity(WhitePoint::D65.tristimulus());
        assert!((x - 0.3127).abs() < 1e-4, "D65 x is {x}");
        assert!((y - 0.3290).abs() < 1e-4, "D65 y is {y}");
    }
}
