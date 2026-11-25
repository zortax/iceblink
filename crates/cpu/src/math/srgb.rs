// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// Ported from iceblink-wgsl's `paint.wgsl`. The Oklab conversion pair and the linear and encoded
// sRGB transfer functions are adapted from that work, which is licensed under the Apache License,
// Version 2.0, and have been modified: the result is premultiplied gamma-encoded sRGB.

//! The sRGB transfer function and the Oklab conversion, which gradient stops are decoded with.

/// Encodes one linear sRGB channel.
pub(crate) fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        return value * 12.92;
    }
    1.055 * value.powf(1.0 / 2.4) - 0.055
}

/// Returns linear sRGB of an Oklab colour.
// Reference: https://bottosson.github.io/posts/oklab/
pub(crate) fn oklab_to_linear_srgb(color: [f32; 3]) -> [f32; 3] {
    let l_ = color[0] + 0.396_337_78 * color[1] + 0.215_803_76 * color[2];
    let m_ = color[0] - 0.105_561_346 * color[1] - 0.063_854_17 * color[2];
    let s_ = color[0] - 0.089_484_18 * color[1] - 1.291_485_5 * color[2];
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}
