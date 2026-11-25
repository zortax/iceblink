//! Chromatic adaptation between the D50 and D65 white points, by the Bradford transform of CSS
//! Color 4.

use crate::color::convert::matrix::{Matrix3, apply};

/// The Bradford transform from D65-referenced XYZ to D50-referenced XYZ.
const D65_TO_D50: Matrix3 = [
    [1.047_929_8, 0.022_946_793, -0.050_192_23],
    [0.029_627_816, 0.990_434_5, -0.017_073_825],
    [-0.009_243_058, 0.015_055_145, 0.751_874_3],
];

/// The Bradford transform from D50-referenced XYZ to D65-referenced XYZ.
const D50_TO_D65: Matrix3 = [
    [0.955_473_4, -0.023_098_537, 0.063_259_31],
    [-0.028_369_707, 1.009_995_5, 0.021_041_399],
    [0.012_314_002, -0.020_507_697, 1.330_365_9],
];

/// Adapts D65-referenced XYZ to D50.
pub(crate) fn d65_to_d50(xyz: [f32; 3]) -> [f32; 3] {
    apply(&D65_TO_D50, xyz)
}

/// Adapts D50-referenced XYZ to D65.
pub(crate) fn d50_to_d65(xyz: [f32; 3]) -> [f32; 3] {
    apply(&D50_TO_D65, xyz)
}

#[cfg(test)]
mod tests {
    use super::{d50_to_d65, d65_to_d50};
    use crate::color::space::WhitePoint;

    #[test]
    fn each_white_point_maps_to_the_other() {
        let adapted = d65_to_d50(WhitePoint::D65.tristimulus());
        let expected = WhitePoint::D50.tristimulus();
        for channel in 0..3 {
            assert!(
                (adapted[channel] - expected[channel]).abs() < 1e-4,
                "D65 white adapted to channel {channel} = {}",
                adapted[channel],
            );
        }

        let adapted = d50_to_d65(WhitePoint::D50.tristimulus());
        let expected = WhitePoint::D65.tristimulus();
        for channel in 0..3 {
            assert!(
                (adapted[channel] - expected[channel]).abs() < 1e-4,
                "D50 white adapted to channel {channel} = {}",
                adapted[channel],
            );
        }
    }
}
