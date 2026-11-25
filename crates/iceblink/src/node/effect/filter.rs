//! Filters of effect groups and backdrops.

use crate::id::{ParamsId, ShaderId};
use crate::premultiplied::Color;

/// One filter of a chain, applied to premultiplied colour.
///
/// Lengths are in device pixels and are not affected by any transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    /// A Gaussian blur with this standard deviation. Zero and below are the identity.
    Blur(f32),
    /// A blurred, offset, tinted copy of the content's alpha, drawn beneath it.
    DropShadow {
        /// How far the copy is moved, as `[x, y]`.
        offset: [f32; 2],
        /// The standard deviation of the copy's blur.
        std_dev: f32,
        /// The tint of the copy.
        color: Color,
    },
    /// A 4x5 matrix in row-major order, applied to straight colour.
    ///
    /// Each output channel is `m[0] r + m[1] g + m[2] b + m[3] a + m[4]` for its row. Results are
    /// clamped to `[0, 1]` and premultiplied again, and a pixel with zero alpha stays transparent.
    ColorMatrix([f32; 20]),
    /// A multiplier on every premultiplied channel, clamped to `[0, 1]`.
    Opacity(f32),
    /// An application shader in filter mode.
    Shader {
        /// The application shader applied as the filter.
        shader: ShaderId,
        /// The shader's parameter block.
        params: ParamsId,
        /// How far outside its rectangle the shader reads.
        reach: f32,
    },
}

/// How many standard deviations of a Gaussian blur are visible.
// Three is where the tail falls below half a level at eight bits per channel.
const BLUR_EXTENT: f32 = 3.0;

/// How far a blur's resampling reads past its kernel.
// A blur runs at half resolution, and each resampling step reads one half-resolution texel past
// the pixel it writes.
const RESAMPLE_REACH: f32 = 2.0;

impl Filter {
    /// Returns how far outside a rectangle the filter reads, as `[left, top, right, bottom]`.
    pub(crate) fn support(&self) -> [f32; 4] {
        match *self {
            Self::Blur(deviation) if deviation > 0.0 => {
                let reach = BLUR_EXTENT * deviation + RESAMPLE_REACH;
                [reach; 4]
            }
            Self::DropShadow {
                offset: [x, y],
                std_dev,
                ..
            } => {
                // The copy is always blurred, so it always resamples.
                let reach = BLUR_EXTENT * std_dev.max(0.0) + RESAMPLE_REACH;
                [
                    (reach - x).max(0.0),
                    (reach - y).max(0.0),
                    (reach + x).max(0.0),
                    (reach + y).max(0.0),
                ]
            }
            Self::Shader { reach, .. } => [reach.max(0.0); 4],
            Self::Blur(_) | Self::ColorMatrix(_) | Self::Opacity(_) => [0.0; 4],
        }
    }
}
