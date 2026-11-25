//! Full-colour sprites: colour glyphs and images, with premultiplied texels.

use iceblink::geom::{Device, Rect};
use iceblink::{ColorSprite, SpriteFlags};

use crate::draw::footprint::{footprint, holds};
use crate::draw::sprite::{sample_color, texel_of, tile_rect};
use crate::draw::{Scene, paint_area};
use crate::math::Floor as _;
use crate::math::saturate;
use crate::math::sdf::rect_coverage;
use crate::math::text::brightness;
use crate::math::vec::{scale4, straight_rgb};
use crate::pages::Page;
use crate::tables::Painter;
use crate::target::Surface;

/// Draws `sprites` in order into `surface`, cut to `scissor`.
pub(crate) fn draw_color_sprites(
    scene: &Scene<'_>,
    surface: &mut Surface<'_>,
    scissor: Rect<i32, Device>,
    sprites: &[ColorSprite],
    row: &mut Vec<[f32; 4]>,
) {
    let Some(within) = scissor.intersection(surface.extent()) else {
        return;
    };
    for sprite in sprites {
        let painted = painted(sprite);
        let space = scene.space(sprite.space);
        let Some(area) = footprint(space, painted, within) else {
            continue;
        };
        let Some(page) = scene.pages.get(sprite.tile.page) else {
            continue;
        };
        let clip = scene.clip(sprite.clip);
        let paint = Painter::of(scene.paints, sprite.paint);
        paint_area(surface, area, row, |device| {
            let Some(point) = space.local(device) else {
                return [0.0; 4];
            };
            if !holds(painted, point) {
                return [0.0; 4];
            }
            let coverage = scene.clip_coverage(clip, device);
            if coverage <= 0.0 {
                return [0.0; 4];
            }
            scale4(
                shade(sprite, page, &paint, point, space.steps(device)),
                coverage,
            )
        });
    }
}

// The rectangle the sprite paints: the frame on a repeating axis, and the bounds cut to the frame
// on the other.
fn painted(sprite: &ColorSprite) -> [f32; 4] {
    let span = |bounds_start: f32, bounds_size: f32, frame_start: f32, frame_size: f32, repeats| {
        if repeats {
            return (frame_start, frame_start + frame_size);
        }
        let low = bounds_start.max(frame_start);
        let high = (bounds_start + bounds_size)
            .min(frame_start + frame_size)
            .max(low);
        (low, high)
    };
    let (b, f) = (sprite.bounds, sprite.frame);
    let x = span(
        b[0],
        b[2],
        f[0],
        f[2],
        sprite.flags.contains(SpriteFlags::REPEAT_X),
    );
    let y = span(
        b[1],
        b[3],
        f[1],
        f[3],
        sprite.flags.contains(SpriteFlags::REPEAT_Y),
    );
    [x.0, y.0, x.1 - x.0, y.1 - y.0]
}

// The sprite's colour at the local point `point`, before its clip.
fn shade(
    sprite: &ColorSprite,
    page: &Page,
    paint: &Painter<'_>,
    point: [f32; 2],
    steps: [[f32; 2]; 2],
) -> [f32; 4] {
    let flags = sprite.flags;
    let rect = tile_rect(&sprite.tile);
    let extent = [sprite.bounds[2].max(1.0e-6), sprite.bounds[3].max(1.0e-6)];
    let local = [point[0] - sprite.bounds[0], point[1] - sprite.bounds[1]];
    let (_, [dx, dy]) = texel_of(local, extent, rect, steps);
    let largest = (dx[0] * dx[0] + dx[1] * dx[1]).max(dy[0] * dy[0] + dy[1] * dy[1]);
    let mut lod = 0.5 * largest.max(1.0).log2();

    let mut uv = [local[0] / extent[0], local[1] / extent[1]];
    if flags.contains(SpriteFlags::REPEAT_X) {
        uv[0] -= uv[0].floored();
    }
    if flags.contains(SpriteFlags::REPEAT_Y) {
        uv[1] -= uv[1].floored();
    }
    if uv[0] < 0.0 || uv[1] < 0.0 || uv[0] > 1.0 || uv[1] > 1.0 {
        return [0.0; 4];
    }
    let mut texel_at = [rect[0] + uv[0] * rect[2], rect[1] + uv[1] * rect[3]];
    if flags.contains(SpriteFlags::NEAREST) {
        texel_at = [
            texel_at[0].min(rect[0] + rect[2] - 0.5).floored() + 0.5,
            texel_at[1].min(rect[1] + rect[3] - 0.5).floored() + 0.5,
        ];
        lod = 0.0;
    }
    let texel = sample_color(page, texel_at, rect, lod);
    let mut color = texel;
    if flags.contains(SpriteFlags::MASK) {
        color = scale4(paint.at(point, sprite.paint_origin), texel[3]);
    } else if flags.contains(SpriteFlags::GRAYSCALE) {
        let gray = brightness(straight_rgb(texel));
        color = [gray * texel[3], gray * texel[3], gray * texel[3], texel[3]];
    }
    if flags.contains(SpriteFlags::PAINT_ALPHA) {
        color = scale4(color, paint.at(point, sprite.paint_origin)[3]);
    }
    let frame = [
        sprite.frame[0] - sprite.bounds[0],
        sprite.frame[1] - sprite.bounds[1],
        sprite.frame[2],
        sprite.frame[3],
    ];
    let framed = rect_coverage(local, frame, &sprite.radii, &sprite.shapes);
    scale4(color, saturate(sprite.opacity) * framed)
}
