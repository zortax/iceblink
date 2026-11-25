//! The shading mathematics of the reference GPU renderer, ported function by function.

pub(crate) mod blend;
pub(crate) mod edge;
pub(crate) mod sdf;
pub(crate) mod srgb;
pub(crate) mod text;
pub(crate) mod vec;

/// Clamps `value` to `[0, 1]`, as WGSL's `saturate` does.
pub(crate) fn saturate(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// Returns `a - b × trunc(a / b)`, a modulus with the sign of `a`.
pub(crate) fn fmod(a: f32, b: f32) -> f32 {
    a - b * (a / b).trunc()
}

/// Rounding down to an integer.
pub(crate) trait Floor {
    /// Returns the largest integer at or below the value, or the value itself when it is already
    /// integral or not a number.
    fn floored(self) -> Self;
}

impl Floor for f32 {
    // `f32::floor` calls libm's `floorf` on x86-64 without SSE4.1, and it showed in profiles.
    fn floored(self) -> Self {
        if self.is_nan() || self.abs() >= 8_388_608.0 {
            return self;
        }
        let truncated = self as i32 as f32;
        if truncated > self {
            truncated - 1.0
        } else {
            truncated
        }
    }
}
