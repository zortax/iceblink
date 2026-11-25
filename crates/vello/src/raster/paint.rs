//! Frame paints as path-renderer brushes.

use iceblink::{FrameGradient, FramePaint, GradientShape};
use iceblink_wgpu::vector::paint::{srgb_stops, straight};
use peniko::color::{AlphaColor, DynamicColor, Srgb};
use peniko::{Brush, ColorStop, Extend, Gradient, InterpolationAlphaSpace};

/// A brush, and the transform of the brush alone when the paint's geometry needs one.
pub(crate) struct Painted {
    pub(crate) brush: Brush,
    /// The map from the gradient's geometry into paint space, for an elliptical radial ramp.
    pub(crate) transform: Option<kurbo::Affine>,
}

/// Returns the brush of `paint`, or `None` for a paint that paints nothing.
///
/// The brush depends on the paint:
///
/// - A solid paint is a solid brush of its straight colour.
/// - A `Sides` paint is a solid brush of its top colour.
/// - A gradient is a gradient brush over the stops [`srgb_stops`] gives, interpolated in
///   premultiplied sRGB. `stops` is scratch for them.
pub(crate) fn brush(paint: &FramePaint, stops: &mut Vec<(f32, [f32; 4])>) -> Option<Painted> {
    match paint {
        FramePaint::None => None,
        FramePaint::Solid(color) => Some(solid(straight(*color))),
        FramePaint::Sides(colors) => Some(solid(straight(colors[0]))),
        FramePaint::Gradient(gradient) => Some(ramp(gradient, stops)),
    }
}

/// Returns a solid brush of the straight sRGB colour `color`.
fn solid(color: [f32; 4]) -> Painted {
    Painted {
        brush: Brush::Solid(AlphaColor::<Srgb>::new(color)),
        transform: None,
    }
}

/// Returns the gradient brush of `gradient`.
fn ramp(gradient: &FrameGradient, stops: &mut Vec<(f32, [f32; 4])>) -> Painted {
    srgb_stops(gradient, stops);
    let ramp: Vec<ColorStop> = stops
        .iter()
        .map(|&(offset, color)| ColorStop {
            offset,
            color: DynamicColor::from_alpha_color(AlphaColor::<Srgb>::new(color)),
        })
        .collect();
    let (shape, transform) = shape(gradient.shape);
    // A conic ramp wraps around its turn whether or not it repeats.
    let wraps = gradient.repeat || matches!(gradient.shape, GradientShape::Conic { .. });
    let extend = if wraps { Extend::Repeat } else { Extend::Pad };
    let mut gradient = shape.with_stops(ramp.as_slice()).with_extend(extend);
    gradient.interpolation_alpha_space = InterpolationAlphaSpace::Premultiplied;
    Painted {
        brush: Brush::Gradient(gradient),
        transform,
    }
}

/// Returns the gradient of `shape`, and the brush transform an elliptical radial one needs.
fn shape(shape: GradientShape) -> (Gradient, Option<kurbo::Affine>) {
    let point = |[x, y]: [f32; 2]| kurbo::Point::new(f64::from(x), f64::from(y));
    match shape {
        GradientShape::Linear { start, end } => {
            (Gradient::new_linear(point(start), point(end)), None)
        }
        GradientShape::Radial { center, radius } => {
            let [x, y] = radius.map(|radius| radius.max(1.0e-6));
            let origin = point(center);
            // The brush has one radius, so the vertical one is a scale about the centre.
            let scale = f64::from(y / x);
            let transform = (scale != 1.0 && scale.is_finite()).then(|| {
                kurbo::Affine::translate(origin.to_vec2())
                    * kurbo::Affine::scale_non_uniform(1.0, scale)
                    * kurbo::Affine::translate(-origin.to_vec2())
            });
            (Gradient::new_radial(origin, x), transform)
        }
        GradientShape::Conic { center, angle } => {
            // The path renderer sweeps from the positive x axis, a quarter turn before twelve
            // o'clock.
            let start = angle - core::f32::consts::FRAC_PI_2;
            (
                Gradient::new_sweep(point(center), start, start + core::f32::consts::TAU),
                None,
            )
        }
    }
}
