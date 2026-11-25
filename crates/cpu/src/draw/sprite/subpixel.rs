//! Per-channel coverage sprites, which blend correctly only against an opaque destination.

use iceblink::Sprite;
use iceblink::geom::{Device, Rect};

use crate::draw::Scene;
use crate::draw::footprint::{footprint, holds};
use crate::draw::sprite::{sample_coverage, texel_of, tile_rect};
use crate::math::vec::straight_rgb;
use crate::pages::Page;
use crate::tables::{Clip, Painter};
use crate::target::Surface;

/// Draws `sprites` in order into `surface`, cut to `scissor`.
///
/// Each channel's coverage is the blend factor of the straight colour, and alpha is left as it
/// is. `row` and `coverage` are scratch for one row.
pub(crate) fn draw_subpixel_sprites(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    sprites: &[Sprite],
    row: &mut Vec<[f32; 4]>,
    coverage: &mut Vec<[f32; 3]>,
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
        let held = Held {
            sprite,
            page,
            clip: scene.clip(sprite.clip),
            paint: Painter::of(scene.paints, sprite.paint),
            rect: tile_rect(&sprite.tile),
            extent: [sprite.bounds[2], sprite.bounds[3]],
        };
        for y in area.top()..area.bottom() {
            row.clear();
            coverage.clear();
            for x in area.left()..area.right() {
                let device = [x as f32 + 0.5, y as f32 + 0.5];
                let (color, factor) = shade(scene, &held, device);
                row.push(color);
                coverage.push(factor);
            }
            surface.subpixel_row(area.left(), y, row, coverage);
        }
    }
}

// A sprite with what drawing it reads.
struct Held<'a> {
    sprite: &'a Sprite,
    page: &'a Page,
    clip: Option<&'a Clip>,
    paint: Painter<'a>,
    rect: [f32; 4],
    extent: [f32; 2],
}

// The straight colour and the per-channel coverage at `device`.
fn shade(scene: &Scene<'_>, held: &Held<'_>, device: [f32; 2]) -> ([f32; 4], [f32; 3]) {
    let Held {
        sprite,
        page,
        clip,
        paint,
        rect,
        extent,
    } = held;
    let (rect, extent) = (*rect, *extent);
    let space = scene.space(sprite.space);
    let Some(point) = space.local(device) else {
        return ([0.0; 4], [0.0; 3]);
    };
    if !holds(sprite.bounds, point) {
        return ([0.0; 4], [0.0; 3]);
    }
    let clipped = scene.clip_coverage(*clip, device);
    let color = paint.at(point, sprite.paint_origin);
    let straight = straight_rgb(color);
    let local = [point[0] - sprite.bounds[0], point[1] - sprite.bounds[1]];
    let (texel, _) = texel_of(local, extent, rect, [[1.0, 0.0], [0.0, 1.0]]);
    let sample = sample_coverage(page, texel, rect);
    let mut stripes = [sample[0], sample[1], sample[2]];
    if scene.blue_to_red {
        stripes.reverse();
    }
    let corrected = scene.gamma.correct3(stripes, straight);
    let factor = corrected.map(|channel| channel * clipped * color[3]);
    ([straight[0], straight[1], straight[2], 1.0], factor)
}
