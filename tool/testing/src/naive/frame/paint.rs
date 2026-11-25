//! Paints as renderers read them.

use std::sync::Arc;

use iceblink::color::{self, ColorSpace, densify};
use iceblink::{Color, FrameGradient, FramePaint, FrameStop, Gradient, Paint, StopEncoding};

/// Returns the frame paint of a client paint.
pub(crate) fn lowered(paint: &Paint) -> FramePaint {
    match paint {
        Paint::Solid(color) => FramePaint::Solid(*color),
        Paint::Sides(colors) => FramePaint::Sides(*colors),
        Paint::Gradient(gradient) => FramePaint::Gradient(gradient_of(gradient)),
    }
}

/// Returns the colour of an inline paint reference.
pub(crate) fn inline(rgba: [u8; 4]) -> FramePaint {
    FramePaint::Solid(Color::from_rgba8(rgba))
}

/// Converts a gradient's stops to the encoding renderers interpolate in.
///
/// sRGB, linear sRGB and Oklab ramps keep their stops in that encoding. Every other space is
/// densified into sRGB stops.
fn gradient_of(gradient: &Gradient) -> FrameGradient {
    let interpolation = gradient.interpolation;
    let (encoding, stops): (StopEncoding, Vec<FrameStop>) = match interpolation.space {
        ColorSpace::Srgb => (
            StopEncoding::SRGB,
            encoded(&gradient.stops, ColorSpace::Srgb),
        ),
        ColorSpace::SrgbLinear => (
            StopEncoding::LINEAR_SRGB,
            encoded(&gradient.stops, ColorSpace::SrgbLinear),
        ),
        ColorSpace::Oklab => (
            StopEncoding::OKLAB,
            encoded(&gradient.stops, ColorSpace::Oklab),
        ),
        _ => {
            let dense = densify(&gradient.stops, interpolation);
            (StopEncoding::SRGB, encoded(&dense, ColorSpace::Srgb))
        }
    };

    FrameGradient {
        shape: gradient.shape,
        encoding,
        repeat: gradient.repeat,
        stops: Arc::from(stops),
    }
}

/// Converts stops into `space`, premultiplied.
///
/// sRGB stops hold the final premultiplied gamma-encoded colours, clamped. Stops of the other
/// encodings keep their channels unclamped.
fn encoded(stops: &[color::GradientStop], space: ColorSpace) -> Vec<FrameStop> {
    stops
        .iter()
        .map(|stop| {
            if space == ColorSpace::Srgb {
                return FrameStop {
                    color: stop.color.to_premultiplied_srgb(),
                    offset: stop.offset,
                };
            }

            let color = stop.color.to_space(space);
            let alpha = color.alpha();
            let [a, b, c] = color.components();
            FrameStop {
                color: [a * alpha, b * alpha, c * alpha, alpha],
                offset: stop.offset,
            }
        })
        .collect()
}
