//! The way an interpolation travels round the hue circle.
//!
//! Half-way between 20° and 320° is 350° the short way round and 170° the long way. The choice is
//! applied as a fixup to the two hues, which are then interpolated as plain numbers.

/// Which way round the hue circle an interpolation travels.
///
/// ```
/// use iceblink::color::HueInterpolation;
///
/// // From 20° to 320°: the short way goes backwards through red.
/// assert_eq!(HueInterpolation::Shorter.fixup(20.0, 320.0), (380.0, 320.0));
/// // The long way goes forwards through green.
/// assert_eq!(HueInterpolation::Longer.fixup(20.0, 320.0), (20.0, 320.0));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HueInterpolation {
    /// The shorter arc, which is the default.
    #[default]
    Shorter,
    /// The longer arc.
    Longer,
    /// The direction of increasing hue.
    Increasing,
    /// The direction of decreasing hue.
    Decreasing,
}

impl HueInterpolation {
    /// Adjusts a pair of hues in degrees, so that interpolating between them as plain numbers
    /// travels the chosen way round the circle.
    ///
    /// Either returned hue may lie outside `0..360`.
    pub fn fixup(self, from: f32, to: f32) -> (f32, f32) {
        let difference = to - from;
        match self {
            Self::Shorter => {
                if difference > 180.0 {
                    (from + 360.0, to)
                } else if difference < -180.0 {
                    (from, to + 360.0)
                } else {
                    (from, to)
                }
            }
            Self::Longer => {
                if difference > 0.0 && difference < 180.0 {
                    (from + 360.0, to)
                } else if difference > -180.0 && difference <= 0.0 {
                    (from, to + 360.0)
                } else {
                    (from, to)
                }
            }
            Self::Increasing => {
                if difference < 0.0 {
                    (from, to + 360.0)
                } else {
                    (from, to)
                }
            }
            Self::Decreasing => {
                if difference > 0.0 {
                    (from + 360.0, to)
                } else {
                    (from, to)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HueInterpolation;

    /// Returns the midpoint of a fixed-up pair, wrapped into `0..360`.
    fn midpoint(method: HueInterpolation, from: f32, to: f32) -> f32 {
        let (from, to) = method.fixup(from, to);
        crate::color::convert::polar::normalize_hue((from + to) / 2.0)
    }

    #[test]
    fn the_short_way_and_the_long_way_are_opposite() {
        assert!((midpoint(HueInterpolation::Shorter, 20.0, 320.0) - 350.0).abs() < 1e-4);
        assert!((midpoint(HueInterpolation::Longer, 20.0, 320.0) - 170.0).abs() < 1e-4);
    }

    #[test]
    fn direction_is_respected_however_far_it_is() {
        assert!((midpoint(HueInterpolation::Increasing, 20.0, 320.0) - 170.0).abs() < 1e-4);
        assert!((midpoint(HueInterpolation::Decreasing, 20.0, 320.0) - 350.0).abs() < 1e-4);
        assert!((midpoint(HueInterpolation::Increasing, 320.0, 20.0) - 350.0).abs() < 1e-4);
        assert!((midpoint(HueInterpolation::Decreasing, 320.0, 20.0) - 170.0).abs() < 1e-4);
    }

    #[test]
    fn a_shorter_crossing_of_zero_goes_the_short_way() {
        assert!((midpoint(HueInterpolation::Shorter, 350.0, 10.0) - 0.0).abs() < 1e-4);
        assert!((midpoint(HueInterpolation::Shorter, 10.0, 350.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn equal_hues_stay_put_except_the_long_way_round() {
        assert_eq!(HueInterpolation::Shorter.fixup(90.0, 90.0), (90.0, 90.0));
        assert_eq!(HueInterpolation::Increasing.fixup(90.0, 90.0), (90.0, 90.0));
        assert_eq!(HueInterpolation::Decreasing.fixup(90.0, 90.0), (90.0, 90.0));
        // Going the long way between two identical hues is a full turn.
        assert_eq!(HueInterpolation::Longer.fixup(90.0, 90.0), (90.0, 450.0));
    }
}
