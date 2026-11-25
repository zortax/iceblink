//! Gradient stops in gamma-encoded sRGB.

use iceblink::FrameGradient;
use iceblink::color::{GradientStop, Interpolation, densify};

use crate::vector::paint::{space, unpremultiplied};

/// Replaces `into` with sRGB stops of `gradient`, as offsets and straight, gamma-encoded sRGB
/// components.
///
/// Interpolated linearly in premultiplied sRGB, the stops draw the gradient's ramp:
///
/// - `SRGB` stops are passed through.
/// - Stops of every other encoding are densified, so the ramp stays within half an eight-bit
///   step of the one their encoding interpolates.
pub fn srgb_stops(gradient: &FrameGradient, into: &mut Vec<(f32, [f32; 4])>) {
    into.clear();
    let Some(space) = space(gradient.encoding) else {
        into.extend(
            gradient
                .stops
                .iter()
                .map(|stop| (stop.offset, unpremultiplied(stop))),
        );
        return;
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
    into.extend(
        densify(&decoded, Interpolation::new(space))
            .iter()
            .map(|stop| {
                let [red, green, blue] = stop.color.components();
                let channels = [red, green, blue, stop.color.alpha()];
                (stop.offset, channels.map(|channel| channel.clamp(0.0, 1.0)))
            }),
    );
}
