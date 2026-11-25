//! The Gaussian's integral, for cases checked against a blur's formula.

/// The standard normal cumulative distribution.
pub(crate) fn phi(t: f32) -> f32 {
    0.5 * (1.0 + erf(t / core::f32::consts::SQRT_2))
}

/// The error function, to within 1.5e-7 (Abramowitz and Stegun 7.1.26).
fn erf(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let poly = t
        * (0.254_829_6
            + t * (-0.284_496_74 + t * (1.421_413_7 + t * (-1.453_152_1 + t * 1.061_405_4))));
    let value = 1.0 - poly * (-x * x).exp();
    value.copysign(x)
}
