//! Paints as one colour.

use iceblink::color::ColorSpace;
use iceblink::{FrameGradient, FramePaint, FrameStop, StopEncoding};

use crate::vector::paint::{space, straight, unpremultiplied};

/// Returns the one straight, gamma-encoded sRGB colour that stands for `paint`, or `None` for a
/// paint that paints nothing.
///
/// The colour depends on the paint:
///
/// - A solid paint is its colour.
/// - A gradient is the mean colour along its ramp, over its stops in sRGB. Each interval between
///   two stops contributes the average of its ends, weighted by its share of the ramp, and the
///   stretches before the first stop and after the last contribute their stop's colour.
/// - A `Sides` paint is its top colour.
pub fn flat(paint: &FramePaint) -> Option<[f32; 4]> {
    match paint {
        FramePaint::None => None,
        FramePaint::Solid(color) => Some(straight(*color)),
        FramePaint::Sides(colors) => Some(straight(colors[0])),
        FramePaint::Gradient(gradient) => mean(gradient),
    }
}

/// Returns the mean colour along the ramp of `gradient`.
fn mean(gradient: &FrameGradient) -> Option<[f32; 4]> {
    let stops = &gradient.stops;
    let first = stops.first()?;
    let last = stops.last()?;
    let color = |stop: &FrameStop| srgb(stop, gradient.encoding);
    let mut total = [0.0_f32; 4];
    let mut covered = 0.0_f32;
    let mut add = |color: [f32; 4], weight: f32| {
        if weight <= 0.0 {
            return;
        }
        for (sum, channel) in total.iter_mut().zip(color) {
            *sum += channel * weight;
        }
        covered += weight;
    };

    add(color(first), first.offset.clamp(0.0, 1.0));
    add(color(last), 1.0 - last.offset.clamp(0.0, 1.0));
    for pair in stops.windows(2) {
        let span = (pair[1].offset - pair[0].offset).clamp(0.0, 1.0);
        let (one, two) = (color(&pair[0]), color(&pair[1]));
        add(
            core::array::from_fn(|channel| (one[channel] + two[channel]) / 2.0),
            span,
        );
    }

    if covered <= 0.0 {
        // Every stop sits at one offset, so the ramp is the first stop everywhere.
        return Some(color(first));
    }
    Some(total.map(|sum| sum / covered))
}

/// Returns the straight, gamma-encoded sRGB colour of `stop`, whose components are in `encoding`.
fn srgb(stop: &FrameStop, encoding: StopEncoding) -> [f32; 4] {
    let [first, second, third, alpha] = unpremultiplied(stop);
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    let Some(space) = space(encoding) else {
        return [first, second, third, alpha].map(|channel| channel.clamp(0.0, 1.0));
    };

    let color = iceblink::color::Color::new(space, [first, second, third], alpha)
        .to_space(ColorSpace::Srgb);
    let [red, green, blue] = color.components();
    [red, green, blue, alpha].map(|channel| channel.clamp(0.0, 1.0))
}
