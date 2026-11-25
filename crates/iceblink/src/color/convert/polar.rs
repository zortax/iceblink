//! Conversion between the rectangular and cylindrical forms of L\*a\*b\*/LCH and Oklab/Oklch.

/// Converts `[lightness, a, b]` to `[lightness, chroma, hue]`, with the hue in degrees.
///
/// The hue of a colour with a chroma below `f32::EPSILON` is zero.
pub(crate) fn to_polar(rectangular: [f32; 3]) -> [f32; 3] {
    let [lightness, a, b] = rectangular;
    let chroma = a.hypot(b);
    let hue = if chroma < f32::EPSILON {
        0.0
    } else {
        normalize_hue(b.atan2(a).to_degrees())
    };
    [lightness, chroma, hue]
}

/// Converts `[lightness, chroma, hue]`, with the hue in degrees, to `[lightness, a, b]`.
pub(crate) fn to_rectangular(polar: [f32; 3]) -> [f32; 3] {
    let [lightness, chroma, hue] = polar;
    let radians = hue.to_radians();
    [lightness, chroma * radians.cos(), chroma * radians.sin()]
}

/// Wraps an angle in degrees into `0..360`.
pub(crate) fn normalize_hue(degrees: f32) -> f32 {
    let wrapped = degrees.rem_euclid(360.0);
    // An angle just below zero rounds up to a whole turn.
    if wrapped >= 360.0 { 0.0 } else { wrapped }
}

#[cfg(test)]
mod tests {
    use super::{normalize_hue, to_polar, to_rectangular};

    #[test]
    fn the_axes_are_where_they_should_be() {
        assert!((to_polar([50.0, 10.0, 0.0])[2] - 0.0).abs() < 1e-4);
        assert!((to_polar([50.0, 0.0, 10.0])[2] - 90.0).abs() < 1e-4);
        assert!((to_polar([50.0, -10.0, 0.0])[2] - 180.0).abs() < 1e-4);
        assert!((to_polar([50.0, 0.0, -10.0])[2] - 270.0).abs() < 1e-4);
    }

    #[test]
    fn a_grey_reports_hue_zero() {
        assert_eq!(to_polar([42.0, 0.0, 0.0]), [42.0, 0.0, 0.0]);
    }

    #[test]
    fn the_two_directions_are_inverses() {
        let rectangular = [56.0, -23.5, 74.25];
        let back = to_rectangular(to_polar(rectangular));
        for channel in 0..3 {
            assert!((back[channel] - rectangular[channel]).abs() < 1e-4);
        }
    }

    #[test]
    fn hues_wrap_into_a_single_turn() {
        assert!((normalize_hue(-90.0) - 270.0).abs() < 1e-4);
        assert!((normalize_hue(450.0) - 90.0).abs() < 1e-4);
        assert!(normalize_hue(0.0).abs() < 1e-6);
    }

    #[test]
    fn an_angle_just_below_zero_stays_inside_the_turn() {
        let hue = normalize_hue(-5e-9);
        assert!((0.0..360.0).contains(&hue), "{hue}");
    }
}
