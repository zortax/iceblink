// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// Ported from iceblink-wgsl's `paint.wgsl`. The ramp sampling and stop decoding are adapted from
// that work, which is licensed under the Apache License, Version 2.0, and have been modified: a
// ramp carries any number of stops, already expressed in the space it is interpolated in.

//! Frame paints evaluated at a point.

use iceblink::{FrameGradient, FramePaint, GradientShape, PaintKind, PaintRef, StopEncoding};

use crate::math::Floor as _;
use crate::math::srgb::{linear_to_srgb, oklab_to_linear_srgb};
use crate::math::vec::mix4;
use crate::math::{fmod, saturate};

/// A paint reference resolved against the paint table, ready to evaluate at many points.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Painter<'a> {
    /// Paints nothing.
    None,
    /// One premultiplied colour everywhere.
    Solid([f32; 4]),
    /// A ramp evaluated per point.
    Gradient(&'a FrameGradient),
    /// The four colours of a border, top first, which paints its top colour away from a border.
    Sides(&'a [iceblink::Color; 4]),
}

impl<'a> Painter<'a> {
    /// Resolves `reference` against `paints`.
    ///
    /// A reference of kind `NONE`, or to a slot past the table, paints nothing.
    pub(crate) fn of(paints: &'a [FramePaint], reference: PaintRef) -> Self {
        if reference.kind == PaintKind::NONE {
            return Self::None;
        }
        match paints.get(reference.index as usize) {
            None | Some(FramePaint::None) => Self::None,
            Some(FramePaint::Solid(color)) => Self::Solid(color.0),
            Some(FramePaint::Gradient(gradient)) => Self::Gradient(gradient),
            Some(FramePaint::Sides(colors)) => Self::Sides(colors),
        }
    }

    /// Returns the premultiplied colour at `point`, with `origin` subtracted first.
    pub(crate) fn at(&self, point: [f32; 2], origin: [f32; 2]) -> [f32; 4] {
        match *self {
            Self::None => [0.0; 4],
            Self::Solid(color) => color,
            Self::Sides(colors) => colors[0].0,
            Self::Gradient(gradient) => {
                let anchored = [point[0] - origin[0], point[1] - origin[1]];
                let raw = position(gradient.shape, anchored);
                let t = if gradient.repeat {
                    raw - raw.floored()
                } else {
                    saturate(raw)
                };
                sample_ramp(gradient, t)
            }
        }
    }

    /// Returns the one colour a solid painter paints everywhere.
    pub(crate) fn solid(&self) -> Option<[f32; 4]> {
        match *self {
            Self::Solid(color) => Some(color),
            _ => None,
        }
    }

    /// Returns the colour of border side `side`, top, right, bottom or left.
    pub(crate) fn side(&self, side: usize) -> [f32; 4] {
        match *self {
            Self::Sides(colors) => colors[side].0,
            _ => [0.0; 4],
        }
    }
}

// Where along a ramp a point lies, before the ramp's extent is applied.
fn position(shape: GradientShape, point: [f32; 2]) -> f32 {
    match shape {
        GradientShape::Linear { start, end } => {
            let axis = [end[0] - start[0], end[1] - start[1]];
            let length_squared = axis[0] * axis[0] + axis[1] * axis[1];
            if length_squared <= 0.0 {
                return 0.0;
            }
            ((point[0] - start[0]) * axis[0] + (point[1] - start[1]) * axis[1]) / length_squared
        }
        GradientShape::Radial { center, radius } => {
            let x = (point[0] - center[0]) / radius[0].max(1e-6);
            let y = (point[1] - center[1]) / radius[1].max(1e-6);
            (x * x + y * y).sqrt()
        }
        GradientShape::Conic { center, angle } => {
            let delta = [point[0] - center[0], point[1] - center[1]];
            let swept = delta[0].atan2(-delta[1]) - angle;
            fmod(swept / (2.0 * core::f32::consts::PI) + 1.0, 1.0)
        }
    }
}

// The ramp's colour at `t`, as premultiplied gamma-encoded sRGB.
fn sample_ramp(gradient: &FrameGradient, t: f32) -> [f32; 4] {
    let stops = &gradient.stops;
    let Some(first) = stops.first() else {
        return [0.0; 4];
    };
    if stops.len() == 1 || t <= first.offset {
        return decode(gradient.encoding, first.color);
    }
    let last = stops[stops.len() - 1];
    if t >= last.offset {
        return decode(gradient.encoding, last.color);
    }
    let mut low = *first;
    for high in &stops[1..] {
        if t <= high.offset {
            let span = (high.offset - low.offset).max(1e-6);
            let mixed = mix4(low.color, high.color, saturate((t - low.offset) / span));
            return decode(gradient.encoding, mixed);
        }
        low = *high;
    }
    decode(gradient.encoding, low.color)
}

// An interpolated stop, back in premultiplied gamma-encoded sRGB.
fn decode(encoding: StopEncoding, value: [f32; 4]) -> [f32; 4] {
    let alpha = value[3];
    if encoding == StopEncoding::SRGB {
        return value;
    }
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    let straight = [value[0] / alpha, value[1] / alpha, value[2] / alpha];
    let linear = if encoding == StopEncoding::OKLAB {
        oklab_to_linear_srgb(straight)
    } else {
        straight
    };
    let [r, g, b] = linear.map(|channel| linear_to_srgb(channel.max(0.0)));
    [r * alpha, g * alpha, b * alpha, alpha]
}
