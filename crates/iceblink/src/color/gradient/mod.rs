//! Gradient stops, and their conversion into sRGB stops a renderer interpolates linearly.
//!
//! A gradient interpolated in a space other than sRGB, such as `in oklab` or `in hsl longer hue`,
//! is a curve in sRGB. [`densify()`] adds stops along that curve, all in
//! [`ColorSpace::Srgb`](crate::color::ColorSpace::Srgb), so a renderer that interpolates between
//! stops in premultiplied sRGB draws the curve to within an eight-bit step.

pub mod densify;

pub use crate::color::gradient::densify::{DEFAULT_TOLERANCE, densify, densify_with_tolerance};

use crate::color::value::Color;

/// A colour at a position along a gradient.
///
/// The offset is a fraction of the gradient line, where `0.0` is its start and `1.0` its end.
/// Offsets outside that range are kept as they are.
///
/// ```
/// use iceblink::color::{Color, GradientStop};
///
/// let start = GradientStop::new(0.0, Color::BLACK);
/// let end = GradientStop::new(1.0, Color::WHITE);
/// assert!(start.offset < end.offset);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    /// Where along the gradient line this stop sits, as a fraction.
    pub offset: f32,
    /// The colour at that position.
    pub color: Color,
}

impl GradientStop {
    /// Creates a stop at `offset` with `color`.
    pub const fn new(offset: f32, color: Color) -> Self {
        Self { offset, color }
    }
}
