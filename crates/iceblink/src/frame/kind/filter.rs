//! Filters of groups and backdrops in a frame.

use crate::node::Filter;
use crate::premultiplied::Color;

/// One filter of a chain in a frame.
///
/// The filters are those of `Filter`, with the shader and its parameters as frame slots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameFilter {
    /// A Gaussian blur with this standard deviation, in device pixels.
    Blur(f32),
    /// A blurred, offset, tinted copy of the content's alpha, drawn beneath it.
    DropShadow {
        /// How far the copy is moved, in device pixels.
        offset: [f32; 2],
        /// The standard deviation of the copy's blur, in device pixels.
        std_dev: f32,
        /// The tint of the copy.
        color: Color,
    },
    /// A 4x5 matrix in row-major order, applied to straight colour.
    ColorMatrix([f32; 20]),
    /// A multiplier on every premultiplied channel.
    Opacity(f32),
    /// An application shader in filter mode.
    Shader {
        /// The frame shader slot.
        shader: u32,
        /// The frame parameter slot.
        params: u32,
        /// How far outside its rectangle the shader reads, in device pixels.
        reach: f32,
    },
}

impl FrameFilter {
    /// Returns how far outside a rectangle the filter reads, as `[left, top, right, bottom]` in
    /// device pixels.
    ///
    /// The supports are those of `read_extent`.
    pub fn support(&self) -> [f32; 4] {
        match *self {
            Self::Blur(deviation) => Filter::Blur(deviation).support(),
            Self::DropShadow {
                offset,
                std_dev,
                color,
            } => Filter::DropShadow {
                offset,
                std_dev,
                color,
            }
            .support(),
            Self::ColorMatrix(matrix) => Filter::ColorMatrix(matrix).support(),
            Self::Opacity(amount) => Filter::Opacity(amount).support(),
            Self::Shader { reach, .. } => [reach.max(0.0); 4],
        }
    }
}
