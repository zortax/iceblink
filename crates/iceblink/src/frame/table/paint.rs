//! Paints in a frame.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::bits::enumeration;
use crate::paint::GradientShape;
use crate::premultiplied::Color;

/// A paint, lowered for renderers.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum FramePaint {
    /// Paints nothing.
    #[default]
    None,
    /// One colour everywhere.
    Solid(Color),
    /// A ramp between stops.
    Gradient(FrameGradient),
    /// One colour per border side: top, right, bottom, left.
    Sides([Color; 4]),
}

impl FramePaint {
    /// Quantises the colour of a solid paint, and each colour of a `Sides` paint, to eight bits
    /// per component with [`Color::quantized`].
    ///
    /// A gradient is unchanged.
    pub fn quantize(&mut self) {
        match self {
            Self::None | Self::Gradient(_) => {}
            Self::Solid(color) => *color = color.quantized(),
            Self::Sides(colors) => *colors = colors.map(Color::quantized),
        }
    }
}

/// A gradient whose stops are in an encoding renderers interpolate.
///
/// A renderer interpolates the stops' premultiplied components linearly and then decodes them to
/// premultiplied gamma-encoded sRGB, as described under "Decoding".
///
/// # Decoding
///
/// `SRGB` needs no decoding. For the other encodings, a zero alpha decodes to transparent.
/// Otherwise the straight components are converted from Oklab to linear sRGB where needed,
/// negative channels are clamped to zero, the sRGB transfer function is applied, and the result
/// is premultiplied again.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameGradient {
    /// The geometry.
    pub shape: GradientShape,
    /// The encoding of the stops.
    pub encoding: StopEncoding,
    /// Whether the ramp repeats outside its extent.
    pub repeat: bool,
    /// The stops, in non-decreasing offset order.
    pub stops: Arc<[FrameStop]>,
}

/// One stop of a lowered gradient.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct FrameStop {
    /// The three premultiplied components of the stop encoding, then alpha.
    pub color: [f32; 4],
    /// Where along the ramp the stop sits.
    pub offset: f32,
}

enumeration! {
    /// The encoding of a lowered gradient's stops.
    ///
    /// A gradient interpolated in sRGB, linear sRGB or Oklab keeps its space. Every other space
    /// is densified into `SRGB` stops whose linear interpolation stays within half an eight-bit
    /// step of the true ramp.
    pub struct StopEncoding(u8) {
        /// Gamma-encoded sRGB.
        SRGB = 0,
        /// Oklab.
        OKLAB = 1,
        /// sRGB primaries with a linear transfer function.
        LINEAR_SRGB = 2,
    }
}
