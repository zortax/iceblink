//! Colour ramps.

use std::sync::Arc;

pub use crate::color::{ColorSpace, GradientStop, HueInterpolation, Interpolation};

/// A colour ramp along a line, an ellipse or a sweep.
///
/// The colour between two stops is interpolated in `interpolation.space` with premultiplied
/// alpha, and is then converted to premultiplied gamma-encoded sRGB. Two stops at one offset make
/// a hard transition.
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// The geometry that maps a point to a position on the ramp.
    pub shape: GradientShape,
    /// The stops, in non-decreasing offset order.
    pub stops: Arc<[GradientStop]>,
    /// The interpolation space, and the hue direction in polar spaces.
    pub interpolation: Interpolation,
    /// Whether the ramp repeats outside its extent. Otherwise the end colours extend.
    pub repeat: bool,
}

/// The geometry of a gradient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientShape {
    /// A ramp from `start` at offset 0 to `end` at offset 1.
    Linear {
        /// The point at offset 0.
        start: [f32; 2],
        /// The point at offset 1.
        end: [f32; 2],
    },
    /// A ramp from `center` outwards.
    Radial {
        /// The point at offset 0.
        center: [f32; 2],
        /// The radii of the ellipse at offset 1.
        radius: [f32; 2],
    },
    /// A ramp swept clockwise around `center`.
    Conic {
        /// The centre of the sweep.
        center: [f32; 2],
        /// The angle of offset 0, in radians clockwise from twelve o'clock.
        angle: f32,
    },
}
