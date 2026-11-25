//! Paints as the frame paint table carries them.

use std::sync::Arc;

use iceblink::color::{self, ColorSpace};
use iceblink::{FrameGradient, FramePaint, FrameStop, Gradient, Paint, StopEncoding};

/// Converts a paint into its frame form, with gradient stops in an encoding renderers interpolate.
pub(crate) fn frame_paint(paint: &Paint) -> FramePaint {
    match paint {
        Paint::Solid(color) => FramePaint::Solid(*color),
        Paint::Sides(colors) => FramePaint::Sides(*colors),
        Paint::Gradient(gradient) => FramePaint::Gradient(frame_gradient(gradient)),
    }
}

/// Converts a gradient, passing sRGB, linear sRGB and Oklab ramps as they are and densifying
/// every other space into sRGB stops.
fn frame_gradient(gradient: &Gradient) -> FrameGradient {
    let (encoding, stops): (StopEncoding, Arc<[FrameStop]>) = match gradient.interpolation.space {
        ColorSpace::Srgb => (
            StopEncoding::SRGB,
            encode(&gradient.stops, ColorSpace::Srgb),
        ),
        ColorSpace::SrgbLinear => (
            StopEncoding::LINEAR_SRGB,
            encode(&gradient.stops, ColorSpace::SrgbLinear),
        ),
        ColorSpace::Oklab => (
            StopEncoding::OKLAB,
            encode(&gradient.stops, ColorSpace::Oklab),
        ),
        _ => {
            let dense = color::densify(&gradient.stops, gradient.interpolation);
            (StopEncoding::SRGB, encode(&dense, ColorSpace::Srgb))
        }
    };

    FrameGradient {
        shape: gradient.shape,
        encoding,
        repeat: gradient.repeat,
        stops,
    }
}

/// Converts stops into premultiplied components of `space`, followed by alpha.
fn encode(stops: &[color::GradientStop], space: ColorSpace) -> Arc<[FrameStop]> {
    stops
        .iter()
        .map(|stop| FrameStop {
            color: premultiplied(stop.color, space),
            offset: stop.offset,
        })
        .collect()
}

/// Returns `color` in `space` with its components premultiplied, followed by alpha.
fn premultiplied(color: color::Color, space: ColorSpace) -> [f32; 4] {
    if space == ColorSpace::Srgb {
        return color.to_premultiplied_srgb();
    }

    let converted = color.to_space(space);
    let alpha = converted.alpha().clamp(0.0, 1.0);
    let [a, b, c] = converted.components();
    [a * alpha, b * alpha, c * alpha, alpha]
}
