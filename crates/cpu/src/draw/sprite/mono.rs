//! Single-channel coverage sprites: glyphs and path masks.

use iceblink::Sprite;
use iceblink::geom::{Device, Rect};

use crate::draw::footprint::{footprint, holds};
use crate::draw::sprite::{sample_coverage, texel_of, tile_rect};
use crate::draw::{Scene, paint_area};
use crate::math::Floor as _;
use crate::math::vec::{mix4, scale4, straight_rgb};
use crate::pages::Page;
use crate::tables::Painter;
use crate::target::Surface;

// How far from one a texel step may be before the tile is resampled.
const RESAMPLE_SLACK: f32 = 0.01;

/// Draws `sprites` in order into `surface`, cut to `scissor`.
pub(crate) fn draw_mono_sprites(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    sprites: &[Sprite],
    row: &mut Vec<[f32; 4]>,
) {
    let Some(within) = scissor.intersection(surface.extent()) else {
        return;
    };
    for sprite in sprites {
        let space = scene.space(sprite.space);
        let Some(area) = footprint(space, sprite.bounds, within) else {
            continue;
        };
        let Some(page) = scene.pages.get(sprite.tile.page) else {
            continue;
        };
        let clip = scene.clip(sprite.clip);
        let paint = Painter::of(scene.paints, sprite.paint);
        let rect = tile_rect(&sprite.tile);
        let extent = [sprite.bounds[2], sprite.bounds[3]];
        paint_area(surface, area, row, |device| {
            let Some(point) = space.local(device) else {
                return [0.0; 4];
            };
            if !holds(sprite.bounds, point) {
                return [0.0; 4];
            }
            let coverage = scene.clip_coverage(clip, device);
            if coverage <= 0.0 {
                return [0.0; 4];
            }
            let local = [point[0] - sprite.bounds[0], point[1] - sprite.bounds[1]];
            let (texel, [dx, dy]) = texel_of(local, extent, rect, space.steps(device));
            let step = [dx[0].abs() + dy[0].abs(), dx[1].abs() + dy[1].abs()];
            let sample = if (step[0] - 1.0).abs() <= RESAMPLE_SLACK
                && (step[1] - 1.0).abs() <= RESAMPLE_SLACK
            {
                sample_coverage(page, texel, rect)[0]
            } else {
                resampled(page, texel, rect)
            };
            let color = paint.at(point, sprite.paint_origin);
            let corrected = scene.gamma.correct(sample, straight_rgb(color));
            scale4(color, corrected * coverage)
        });
    }
}

// The four texels around `texel`, each read at its centre and weighted by distance.
fn resampled(page: &Page, texel: [f32; 2], rect: [f32; 4]) -> f32 {
    let at = [texel[0] - 0.5, texel[1] - 0.5];
    let base = [at[0].floored() + 0.5, at[1].floored() + 0.5];
    let weight = [at[0] - at[0].floored(), at[1] - at[1].floored()];
    let read = |dx: f32, dy: f32| sample_coverage(page, [base[0] + dx, base[1] + dy], rect);
    let low = mix4(read(0.0, 0.0), read(1.0, 0.0), weight[0]);
    let high = mix4(read(0.0, 1.0), read(1.0, 1.0), weight[0]);
    mix4(low, high, weight[1])[0]
}
