//! Frame paints as the straight colours a scratch holds.

mod mean;
mod stops;

use iceblink::color::ColorSpace;
use iceblink::{Color, FramePaint, FrameStop, PaintKind, PaintRef, StopEncoding};

pub use self::mean::flat;
pub use self::stops::srgb_stops;

/// Returns the straight, gamma-encoded sRGB components of a premultiplied colour.
///
/// A colour with zero alpha is transparent black.
pub fn straight(color: Color) -> [f32; 4] {
    let [red, green, blue, alpha] = color.0;
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    [red / alpha, green / alpha, blue / alpha, alpha].map(|channel| channel.clamp(0.0, 1.0))
}

/// Returns the paint `reference` refers to in `paints`, or `None` for a reference of kind
/// [`PaintKind::NONE`] or a slot past the table.
pub fn resolve(reference: PaintRef, paints: &[FramePaint]) -> Option<&FramePaint> {
    if reference.kind == PaintKind::NONE {
        return None;
    }
    paints.get(reference.index as usize)
}

/// Returns the colour space of stops in `encoding`, or `None` for gamma-encoded sRGB stops.
fn space(encoding: StopEncoding) -> Option<ColorSpace> {
    match encoding {
        StopEncoding::OKLAB => Some(ColorSpace::Oklab),
        StopEncoding::LINEAR_SRGB => Some(ColorSpace::SrgbLinear),
        _ => None,
    }
}

/// Returns the straight components of a stop, in the stop's encoding.
///
/// A stop with zero alpha is transparent black.
fn unpremultiplied(stop: &FrameStop) -> [f32; 4] {
    let [first, second, third, alpha] = stop.color;
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    [first / alpha, second / alpha, third / alpha, alpha]
}
