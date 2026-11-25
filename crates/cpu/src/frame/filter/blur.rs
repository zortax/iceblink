// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// Ported from iceblink-wgsl's `blur.wgsl`. The separable gaussian chain, a snapped 2:1 box
// downsample followed by two single-axis passes whose taps spread out when the radius outruns the
// tap budget, is adapted from that work, which is licensed under the Apache License, Version 2.0,
// and has been modified: the sampling lattice is anchored to the device origin.

//! The blur of the reference renderer: a half-resolution downsample, then a horizontal and a
//! vertical pass of at most sixteen taps a side.

use iceblink::geom::{Device, Edges, Point, Rect};

use crate::frame::filter::EdgeMode;
use crate::frame::pool::Pool;
use crate::target::{Canvas, Precision, Texture};

// Three deviations is where a gaussian's tail falls below half a level at eight bits.
const BLUR_EXTENT: f32 = 3.0;

const MAX_TAPS: f32 = 16.0;

// Each pass writes this far past its region for the next, cut to what the source holds.
const MARGIN: i32 = 2;

const SLACK: i32 = 4;

/// Returns how far a blur of `deviation` reads past the pixel it writes.
pub(crate) fn reach(deviation: f32) -> i32 {
    (BLUR_EXTENT * deviation.max(0.0)).ceil() as i32
}

/// Returns `source` blurred by `deviation` device pixels over `output`, at half resolution.
///
/// `valid` is what `source` holds, and reads outside it return what `edge` says.
pub(crate) fn blur(
    pool: &mut Pool,
    source: &Texture<'_>,
    output: Rect<i32, Device>,
    valid: Rect<i32, Device>,
    deviation: f32,
    edge: EdgeMode,
) -> Canvas {
    let reach = reach(deviation);
    let downsampled = grown(output, reach, reach, valid);
    let horizontal = grown(output, 0, reach, valid);
    let promised = |region| grown(region, MARGIN, MARGIN, valid);
    let mut ping = pool.acquire(valid, Precision::Float, true);
    let mut pong = pool.acquire(valid, Precision::Float, true);

    let source_valid = texels_at(valid, source.origin(), source.scale());
    downsample(
        source,
        Bounds {
            valid: source_valid,
            edge,
        },
        &mut ping,
        with_slack(downsampled, valid),
    );
    let written = texels_at(promised(downsampled), ping.region().origin, 0.5);
    axis(
        &ping.texture(),
        Bounds {
            valid: written,
            edge,
        },
        &mut pong,
        with_slack(horizontal, valid),
        [1.0, 0.0],
        deviation,
    );
    let written = texels_at(promised(horizontal), pong.region().origin, 0.5);
    axis(
        &pong.texture(),
        Bounds {
            valid: written,
            edge,
        },
        &mut ping,
        with_slack(output, valid),
        [0.0, 1.0],
        deviation,
    );
    pool.release(pong);
    ping
}

/// The texels a pass's source holds, and what it reads outside them.
#[derive(Clone, Copy, Debug)]
struct Bounds {
    valid: Rect<i32, Device>,
    edge: EdgeMode,
}

impl Bounds {
    /// Returns `source` sampled at `texel`, bilinearly unless `exact` puts it on a texel centre.
    fn read(self, source: &Texture<'_>, texel: [f32; 2], exact: bool) -> [f32; 4] {
        let at = clamped(texel, self.valid);
        let value = if exact {
            source.load(at[0] as i32, at[1] as i32)
        } else {
            source.bilinear(at)
        };
        match self.edge {
            EdgeMode::Duplicate => value,
            EdgeMode::Transparent => value.map(|channel| channel * inside(texel, self.valid)),
        }
    }
}

// Writes the average of each two by two source texels into the texels of `into` that `region`
// touches.
fn downsample(source: &Texture<'_>, bounds: Bounds, into: &mut Canvas, region: Rect<i32, Device>) {
    let origin = into.region().origin;
    let scale = source.scale();
    let source_origin = source.origin();
    for_texels(into, region, |x, y| {
        let device = [
            (x as f32 + 0.5) * 2.0 + origin.x as f32,
            (y as f32 + 0.5) * 2.0 + origin.y as f32,
        ];
        let texel = [
            (device[0] - source_origin.x as f32) * scale,
            (device[1] - source_origin.y as f32) * scale,
        ];
        bounds.read(source, texel, false)
    });
}

// Writes one axis of the gaussian into the texels of `into` that `region` touches.
fn axis(
    source: &Texture<'_>,
    bounds: Bounds,
    into: &mut Canvas,
    region: Rect<i32, Device>,
    direction: [f32; 2],
    deviation: f32,
) {
    let texels = (deviation * 0.5).max(0.0);
    let reach = BLUR_EXTENT * texels;
    let spacing = (reach / MAX_TAPS).max(1.0);
    let taps = (reach / spacing).ceil().clamp(1.0, MAX_TAPS) as i32;
    let sigma = texels.max(1e-4);
    let weights: Vec<f32> = (-taps..=taps)
        .map(|tap| gaussian(tap as f32 * spacing, sigma))
        .collect();
    let total = weights.iter().sum::<f32>().max(1e-5);
    let origin = into.region().origin;
    let source_origin = source.origin();
    for_texels(into, region, |x, y| {
        let device = [
            (x as f32 + 0.5) * 2.0 + origin.x as f32,
            (y as f32 + 0.5) * 2.0 + origin.y as f32,
        ];
        let centre = [
            (device[0] - source_origin.x as f32) * 0.5,
            (device[1] - source_origin.y as f32) * 0.5,
        ];
        let mut sum = [0.0f32; 4];
        // On texel centres with whole steps, every tap reads one texel.
        let along = centre[0] * direction[0] + centre[1] * direction[1];
        let across = centre[0] * direction[1] + centre[1] * direction[0];
        let centred = along.fract() == 0.5 && across.fract() == 0.5 && spacing.fract() == 0.0;
        for (index, weight) in weights.iter().enumerate() {
            let offset = (index as i32 - taps) as f32 * spacing;
            let at = [
                centre[0] + direction[0] * offset,
                centre[1] + direction[1] * offset,
            ];
            let texel = bounds.read(source, at, centred);
            for channel in 0..4 {
                sum[channel] += texel[channel] * weight;
            }
        }
        sum.map(|channel| channel / total)
    });
}

fn gaussian(distance: f32, deviation: f32) -> f32 {
    (-(distance * distance) / (2.0 * deviation * deviation)).exp()
        / ((2.0 * core::f32::consts::PI).sqrt() * deviation)
}

// Calls `value` for each texel of the half-resolution `into` that the device rectangle `region`
// touches, and stores what it returns.
fn for_texels(
    into: &mut Canvas,
    region: Rect<i32, Device>,
    mut value: impl FnMut(usize, usize) -> [f32; 4],
) {
    let origin = into.region().origin;
    let [width, height] = into.texel_size();
    let texels = texels_at(region, origin, 0.5);
    let left = texels.left().clamp(0, width as i32) as usize;
    let right = texels.right().clamp(0, width as i32) as usize;
    let top = texels.top().clamp(0, height as i32) as usize;
    let bottom = texels.bottom().clamp(0, height as i32) as usize;
    for y in top..bottom {
        for x in left..right {
            let texel = value(x, y);
            into.store(x, y, texel);
        }
    }
}

// The texels of a target at `origin` and `scale` that the device rectangle `rect` touches.
pub(crate) fn texels_at(
    rect: Rect<i32, Device>,
    origin: Point<i32, Device>,
    scale: f32,
) -> Rect<i32, Device> {
    let left = rect.left() - origin.x;
    let top = rect.top() - origin.y;
    let right = (rect.right() - origin.x).max(left);
    let bottom = (rect.bottom() - origin.y).max(top);
    if scale >= 1.0 {
        return Rect::from_corners(Point::new(left, top), Point::new(right, bottom));
    }
    Rect::from_corners(
        Point::new(left.div_euclid(2), top.div_euclid(2)),
        Point::new((right + 1).div_euclid(2), (bottom + 1).div_euclid(2)),
    )
}

// A texel position clamped to the inner half-texel of `valid`.
fn clamped(texel: [f32; 2], valid: Rect<i32, Device>) -> [f32; 2] {
    let low = [valid.left() as f32 + 0.5, valid.top() as f32 + 0.5];
    let high = [
        (valid.right() as f32 - 0.5).max(low[0]),
        (valid.bottom() as f32 - 0.5).max(low[1]),
    ];
    [
        texel[0].clamp(low[0], high[0]),
        texel[1].clamp(low[1], high[1]),
    ]
}

// The share of a bilinear tap at `texel` that falls on texels of `valid`.
fn inside(texel: [f32; 2], valid: Rect<i32, Device>) -> f32 {
    let axis = |at: f32, low: i32, high: i32| {
        (at - low as f32 + 0.5).clamp(0.0, 1.0) * (high as f32 + 0.5 - at).clamp(0.0, 1.0)
    };
    axis(texel[0], valid.left(), valid.right()) * axis(texel[1], valid.top(), valid.bottom())
}

fn grown(rect: Rect<i32, Device>, x: i32, y: i32, within: Rect<i32, Device>) -> Rect<i32, Device> {
    rect.outset(Edges::axes(x, y))
        .intersection(within)
        .unwrap_or(rect)
}

fn with_slack(rect: Rect<i32, Device>, within: Rect<i32, Device>) -> Rect<i32, Device> {
    grown(rect, MARGIN + SLACK, MARGIN + SLACK, within)
}
