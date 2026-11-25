// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl (Apache-2.0)
// Ported from iceblink-wgsl's `shadow.wgsl`. The analytic blurred rounded rectangle, the `erf`
// approximation, the closed-form horizontal integral, the four-sample vertical accumulation and the
// inset complement are adapted from that work, which is licensed under the Apache License, Version
// 2.0, and have been modified: the scanline's horizontal extent is the ellipse's or the
// superellipse's own edge.

//! Shadows: a blurred rounded rectangle, cast outside or inside its caster.

use iceblink::FrameShadow;
use iceblink::geom::{Device, Rect};

use crate::draw::footprint::{footprint, holds};
use crate::draw::quad::inflate;
use crate::draw::{Scene, paint_area};
use crate::math::saturate;
use crate::math::sdf::{CORNER_ROUND, pick_corner_radii, pick_corner_shape, quad_sdf};
use crate::math::vec::scale4;
use crate::target::Surface;

const PI: f32 = core::f32::consts::PI;

/// Draws `shadows` in order into `surface`, cut to `scissor`.
pub(crate) fn draw_shadows(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    shadows: &[FrameShadow],
    row: &mut Vec<[f32; 4]>,
) {
    let Some(within) = scissor.intersection(surface.extent()) else {
        return;
    };
    for instance in shadows {
        let inflated = inflate(instance.bounds);
        let space = scene.space(instance.shadow.space.0);
        let Some(area) = footprint(space, inflated, within) else {
            continue;
        };
        let clip = scene.clip(instance.shadow.clip.0);
        let mut rows = Rows::default();
        paint_area(surface, area, row, |device| {
            let Some(point) = space.local(device) else {
                return [0.0; 4];
            };
            if !holds(inflated, point) {
                return [0.0; 4];
            }
            let coverage = scene.clip_coverage(clip, device);
            if coverage <= 0.0 {
                return [0.0; 4];
            }
            scale4(
                instance.shadow.color.0,
                alpha(instance, point, &mut rows) * coverage,
            )
        });
    }
}

// The vertical samples of the last scanline a shadow was evaluated on. Pixels at the same local
// height share them.
#[derive(Clone, Copy, Debug)]
struct Rows {
    y: f32,
    // The offset of each sample from the point, and its weight.
    samples: [(f32, f32); 4],
}

impl Default for Rows {
    fn default() -> Self {
        Self {
            y: f32::NAN,
            samples: [(0.0, 0.0); 4],
        }
    }
}

// The shadow's alpha at the local point `point`.
fn alpha(instance: &FrameShadow, point: [f32; 2], rows: &mut Rows) -> f32 {
    let shadow = &instance.shadow;
    let origin = [instance.bounds[0], instance.bounds[1]];
    let from = |rect: [f32; 4]| [rect[0] - origin[0], rect[1] - origin[1], rect[2], rect[3]];
    let casting = from(shadow.shape_bounds);
    let caster = from(shadow.caster_bounds);
    let local = [point[0] - origin[0], point[1] - origin[1]];
    let half_size = [casting[2] * 0.5, casting[3] * 0.5];
    let center = [casting[0] + half_size[0], casting[1] + half_size[1]];
    let center_to_point = [local[0] - center[0], local[1] - center[1]];
    let corner = pick_corner_radii(center_to_point, &shadow.radii);
    let corner_shape = pick_corner_shape(center_to_point, &shadow.shapes);
    let blur = shadow.blur;

    let caster_distance = quad_sdf(local, caster, &shadow.caster_radii, &shadow.shapes);
    if shadow.inset == 0 && caster_distance <= -0.5 {
        return 0.0;
    }

    let mut alpha = if blur <= 0.0 {
        saturate(0.5 - quad_sdf(local, casting, &shadow.radii, &shadow.shapes))
    } else {
        if rows.y != center_to_point[1] {
            // The samples are spent where three deviations of the gaussian overlap the shape.
            let low = center_to_point[1] - half_size[1];
            let high = center_to_point[1] + half_size[1];
            let start = (-3.0 * blur).clamp(low, high);
            let end = (3.0 * blur).clamp(low, high);
            let step = (end - start) / 4.0;
            let mut y = start + step * 0.5;
            for sample in &mut rows.samples {
                *sample = (y, gaussian(y, blur) * step);
                y += step;
            }
            rows.y = center_to_point[1];
        }
        rows.samples
            .iter()
            .map(|&(y, weight)| {
                let blurred = blur_along_x(
                    center_to_point[0],
                    center_to_point[1] - y,
                    blur,
                    corner,
                    half_size,
                    corner_shape,
                );
                blurred * weight
            })
            .sum()
    };

    if shadow.inset != 0 {
        alpha = (1.0 - alpha) * saturate(0.5 - caster_distance);
    } else {
        alpha *= saturate(0.5 + caster_distance);
    }
    alpha
}

fn gaussian(x: f32, sigma: f32) -> f32 {
    (-(x * x) / (2.0 * sigma * sigma)).exp() / ((2.0 * PI).sqrt() * sigma)
}

// An approximation of the error function.
fn erf(v: f32) -> f32 {
    let s = if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    };
    let a = v.abs();
    let r1 = 1.0 + (0.278_393 + (0.230_389 + (0.000_972 + 0.078_108 * a) * a) * a) * a;
    let r2 = r1 * r1;
    s - s / (r2 * r2)
}

// The blurred coverage of one scanline of a rounded rectangle, analytic in x.
fn blur_along_x(
    x: f32,
    y: f32,
    sigma: f32,
    corner: [f32; 2],
    half_size: [f32; 2],
    shape: f32,
) -> f32 {
    let delta = (half_size[1] - corner[1] - y.abs()).min(0.0);
    let mut curved = half_size[0] - corner[0];
    if corner[1] > 0.0 {
        if shape == CORNER_ROUND {
            let normalised = saturate(1.0 - (delta * delta) / (corner[1] * corner[1]));
            curved += corner[0] * normalised.sqrt();
        } else {
            let power = shape.clamp(0.01, 64.0);
            let along = saturate(delta.abs() / corner[1]);
            let remaining = saturate(1.0 - along.powf(power));
            curved += corner[0] * remaining.powf(1.0 / power);
        }
    } else {
        curved += corner[0];
    }
    let scale = 0.5f32.sqrt() / sigma;
    let low = 0.5 + 0.5 * erf((x - curved) * scale);
    let high = 0.5 + 0.5 * erf((x + curved) * scale);
    high - low
}
