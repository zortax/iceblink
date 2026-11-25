//! Paints and colours.

use iceblink::{Color, GradientShape, Paint};

use super::all;

/// Reports whether the colours of a solid or sided `paint` are valid, and whether every gradient
/// coordinate, stop colour and stop offset is finite.
pub(crate) fn paint(paint: &Paint) -> bool {
    match paint {
        Paint::Solid(solid) => color(solid),
        Paint::Sides(sides) => sides.iter().all(color),
        Paint::Gradient(gradient) => {
            let shape = match gradient.shape {
                GradientShape::Linear { start, end } => all(start.iter().chain(&end)),
                GradientShape::Radial { center, radius } => all(center.iter().chain(&radius)),
                GradientShape::Conic { center, angle } => all(center.iter().chain([&angle])),
            };
            // Stop colours carry straight alpha in their own space and are clamped only when
            // converted, so they need only be finite.
            shape
                && gradient.stops.iter().all(|stop| {
                    let color = stop.color;
                    all(color
                        .components()
                        .iter()
                        .chain([&color.alpha(), &stop.offset]))
                })
        }
    }
}

/// How far a colour channel may lie above its alpha, for premultiplying that rounds up.
const PREMULTIPLIED_SLACK: f32 = 1.0e-4;

/// Reports whether `color` is premultiplied: every component in `[0, 1]`, and no colour channel
/// more than [`PREMULTIPLIED_SLACK`] above its alpha.
pub(crate) fn color(color: &Color) -> bool {
    let [r, g, b, a] = color.0;
    let premultiplied = |channel: &f32| *channel <= a + PREMULTIPLIED_SLACK;
    color
        .0
        .iter()
        .all(|component| (0.0..=1.0).contains(component))
        && [r, g, b].iter().all(premultiplied)
}
