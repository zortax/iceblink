//! Decoration lines: underline, overline and line-through, in five styles.

use iceblink::geom::{Device, Rect};
use iceblink::{Line, LineStyle};

use crate::draw::footprint::{footprint, holds};
use crate::draw::quad::inflate;
use crate::draw::{Scene, paint_area};
use crate::math::vec::scale4;
use crate::math::{fmod, saturate};
use crate::target::Surface;

/// Draws `lines` in order into `surface`, cut to `scissor`.
pub(crate) fn draw_lines(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    lines: &[Line],
    row: &mut Vec<[f32; 4]>,
) {
    let Some(within) = scissor.intersection(surface.extent()) else {
        return;
    };
    for line in lines {
        let inflated = inflate(line.bounds);
        let space = scene.space(line.space.0);
        let Some(area) = footprint(space, inflated, within) else {
            continue;
        };
        let clip = scene.clip(line.clip.0);
        paint_area(surface, area, row, |device| {
            let Some(point) = space.local(device) else {
                return [0.0; 4];
            };
            if !holds(inflated, point) {
                return [0.0; 4];
            }
            let clipped = scene.clip_coverage(clip, device);
            if clipped <= 0.0 {
                return [0.0; 4];
            }
            let local = [point[0] - line.bounds[0], point[1] - line.bounds[1]];
            scale4(line.color.0, coverage(line, local) * clipped)
        });
    }
}

// The line's coverage at `local`, from its bounds' origin.
fn coverage(line: &Line, local: [f32; 2]) -> f32 {
    let size = [line.bounds[2], line.bounds[3]];
    let thickness = line.thickness.max(1.0);
    let ends = saturate(0.5 + local[0].min(size[0] - local[0]));
    let edges = saturate(0.5 + local[1].min(size[1] - local[1]));
    match line.style {
        LineStyle::WAVY => {
            let amplitude = ((size[1] - thickness) * 0.5).max(0.0);
            let wavelength = (thickness * 4.0).max(1.0);
            let wave = size[1] * 0.5
                + amplitude * (local[0] * 2.0 * core::f32::consts::PI / wavelength).sin();
            saturate(0.5 + thickness * 0.5 - (local[1] - wave).abs()) * ends * edges
        }
        LineStyle::DOUBLE => {
            let band = size[1] / 3.0;
            let first = saturate(0.5 + band * 0.5 - (local[1] - band * 0.5).abs());
            let second = saturate(0.5 + band * 0.5 - (local[1] - band * 2.5).abs());
            first.max(second) * ends * edges
        }
        LineStyle::DASHED | LineStyle::DOTTED => {
            let dash = if line.style == LineStyle::DOTTED {
                thickness
            } else {
                thickness * 3.0
            };
            let phase = fmod(local[0], dash * 2.0);
            ends * edges * saturate(0.5 + dash - (phase - dash * 0.5).abs() - dash * 0.5)
        }
        _ => ends * edges,
    }
}
