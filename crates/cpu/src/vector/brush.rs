//! Frame paints as path-rasteriser brushes.

use iceblink::color::{ColorSpace, GradientStop, Interpolation, densify};
use iceblink::{Color, FrameGradient, FramePaint, FrameStop, GradientShape, StopEncoding};
use vello_cpu::peniko::color::{AlphaColor, DynamicColor, Srgb};
use vello_cpu::peniko::{Brush, ColorStop, Extend, Gradient, InterpolationAlphaSpace};

/// A brush, and the map from the gradient's geometry into paint space when it needs one.
pub(crate) struct Painted {
    pub(crate) brush: vello_cpu::PaintType,
    pub(crate) transform: Option<kurbo::Affine>,
}

/// Returns the brush of `paint`, or `None` for a paint that paints nothing.
///
/// A `Sides` paint is its top colour, and a gradient is interpolated in premultiplied sRGB over
/// stops densified from its encoding.
pub(crate) fn brush(paint: &FramePaint) -> Option<Painted> {
    match paint {
        FramePaint::None => None,
        FramePaint::Solid(color) => Some(solid(*color)),
        FramePaint::Sides(colors) => Some(solid(colors[0])),
        FramePaint::Gradient(gradient) => Some(ramp(gradient)),
    }
}

fn solid(color: Color) -> Painted {
    Painted {
        brush: Brush::Solid(AlphaColor::<Srgb>::new(straight(color.0))),
        transform: None,
    }
}

fn ramp(gradient: &FrameGradient) -> Painted {
    let stops = srgb_stops(gradient);
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
            // The rasteriser sweeps from the positive x axis, a quarter turn before twelve
            // o'clock.
            let start = angle - core::f32::consts::FRAC_PI_2;
            (
                Gradient::new_sweep(point(center), start, start + core::f32::consts::TAU),
                None,
            )
        }
    }
}

// The straight, gamma-encoded sRGB components of a premultiplied colour.
fn straight(color: [f32; 4]) -> [f32; 4] {
    let [red, green, blue, alpha] = color;
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    [red / alpha, green / alpha, blue / alpha, alpha].map(|channel| channel.clamp(0.0, 1.0))
}

// The gradient's stops as offsets and straight sRGB components, densified from its encoding.
fn srgb_stops(gradient: &FrameGradient) -> Vec<(f32, [f32; 4])> {
    let space = match gradient.encoding {
        StopEncoding::OKLAB => ColorSpace::Oklab,
        StopEncoding::LINEAR_SRGB => ColorSpace::SrgbLinear,
        _ => {
            return gradient
                .stops
                .iter()
                .map(|stop| (stop.offset, unpremultiplied(stop)))
                .collect();
        }
    };
    let decoded: Vec<GradientStop> = gradient
        .stops
        .iter()
        .map(|stop| {
            let [first, second, third, alpha] = unpremultiplied(stop);
            let color = iceblink::color::Color::new(space, [first, second, third], alpha);
            GradientStop::new(stop.offset, color)
        })
        .collect();
    densify(&decoded, Interpolation::new(space))
        .iter()
        .map(|stop| {
            let [red, green, blue] = stop.color.components();
            let channels = [red, green, blue, stop.color.alpha()];
            (stop.offset, channels.map(|channel| channel.clamp(0.0, 1.0)))
        })
        .collect()
}

fn unpremultiplied(stop: &FrameStop) -> [f32; 4] {
    let [first, second, third, alpha] = stop.color;
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    [first / alpha, second / alpha, third / alpha, alpha]
}
