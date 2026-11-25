// DERIVED-FROM: the Windows Terminal project, src/renderer/atlas/dwrite.hlsl (MIT)
// Copyright (c) Microsoft Corporation.
// Ported from iceblink-wgsl's `text.wgsl`. The contrast enhancement and the cubic alpha correction
// are adapted from that work, which is distributed under the MIT License, and have been modified:
// the coefficients are computed once from the display gamma, and the single-channel and per-channel
// forms share one set of helpers.

//! Contrast enhancement and gamma correction of glyph coverage.

/// The display gamma coverage is corrected for.
const DISPLAY_GAMMA: f32 = 2.2;

/// The contrast enhancement of single-channel coverage.
const GRAYSCALE_CONTRAST: f32 = 0.5;

/// The contrast enhancement of per-channel coverage.
const SUBPIXEL_CONTRAST: f32 = 1.0;

/// The gamma correction of glyph coverage for one display gamma.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextGamma {
    ratios: [f32; 4],
}

impl TextGamma {
    /// Returns the correction for a display gamma of 2.2.
    pub(crate) fn display() -> Self {
        Self {
            ratios: iceblink::color::gamma_correction_ratios(DISPLAY_GAMMA),
        }
    }

    /// Returns single-channel coverage `sample` corrected for text of straight colour `color`.
    pub(crate) fn correct(&self, sample: f32, color: [f32; 3]) -> f32 {
        let contrast = light_on_dark_contrast(GRAYSCALE_CONTRAST, color);
        let contrasted = enhance_contrast(sample, contrast);
        alpha_correction(contrasted, brightness(color), self.ratios)
    }

    /// Returns per-channel coverage `sample` corrected for text of straight colour `color`.
    pub(crate) fn correct3(&self, sample: [f32; 3], color: [f32; 3]) -> [f32; 3] {
        let contrast = light_on_dark_contrast(SUBPIXEL_CONTRAST, color);
        [0, 1, 2].map(|channel| {
            let contrasted = enhance_contrast(sample[channel], contrast);
            alpha_correction(contrasted, color[channel], self.ratios)
        })
    }
}

/// Returns the perceived brightness of a colour, with the luminance coefficients of REC. 601.
pub(crate) fn brightness(color: [f32; 3]) -> f32 {
    color[0] * 0.30 + color[1] * 0.59 + color[2] * 0.11
}

// Brighter text gets less enhancement, and none from a brightness of 0.75 up.
fn light_on_dark_contrast(enhanced: f32, color: [f32; 3]) -> f32 {
    enhanced * (4.0 * (0.75 - brightness(color))).clamp(0.0, 1.0)
}

fn enhance_contrast(alpha: f32, k: f32) -> f32 {
    alpha * (k + 1.0) / (alpha * k + 1.0)
}

fn alpha_correction(a: f32, b: f32, g: [f32; 4]) -> f32 {
    let brightness_adjustment = g[0] * b + g[1];
    let correction = brightness_adjustment * a + (g[2] * b + g[3]);
    a + a * (1.0 - a) * correction
}
