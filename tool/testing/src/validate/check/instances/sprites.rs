//! Mono, subpixel and colour sprites.

use iceblink::{ColorSprite, PageFormat, Sprite, SpriteFlags};

use crate::validate::Rule;
use crate::validate::check::Inspect;

/// How far a glyph sprite may sit from a whole device pixel.
const PIXEL_TOLERANCE: f32 = 1.0e-3;

pub(super) fn mono(inspect: &mut Inspect<'_>, position: usize, sprite: &Sprite) {
    coverage(inspect, "mono", PageFormat::A8, position, sprite);
}

pub(super) fn subpixel(inspect: &mut Inspect<'_>, position: usize, sprite: &Sprite) {
    coverage(inspect, "subpixel", PageFormat::SUBPIXEL, position, sprite);
}

fn coverage(
    inspect: &mut Inspect<'_>,
    lane: &str,
    format: PageFormat,
    position: usize,
    sprite: &Sprite,
) {
    let at = || format!("{lane} source {position}");
    inspect.clip(sprite.clip, at);
    inspect.space(sprite.space, at);
    inspect.paint(sprite.paint, at);
    inspect.tile(sprite.tile, Some(format), at);
    whole_pixels(inspect, sprite, at);
}

/// Checks that a sprite under a translation covers whole device pixels, one texel each.
fn whole_pixels(inspect: &mut Inspect<'_>, sprite: &Sprite, at: impl Fn() -> String) {
    let Some(matrix) = inspect.known.spaces.get(sprite.space).found() else {
        return;
    };
    let Some((dx, dy)) = matrix.plane_translation() else {
        return;
    };

    let [x, y, width, height] = sprite.bounds;
    let off_grid = |value: f32| (value - value.round()).abs() > PIXEL_TOLERANCE;
    if off_grid(x + dx) || off_grid(y + dy) {
        inspect.push(
            Rule::WholePixels,
            format!("{} lands at {}, {}", at(), x + dx, y + dy),
        );
    }

    let texels = [sprite.tile.rect[2] as f32, sprite.tile.rect[3] as f32];
    if (width - texels[0]).abs() > PIXEL_TOLERANCE || (height - texels[1]).abs() > PIXEL_TOLERANCE {
        inspect.push(
            Rule::WholePixels,
            format!(
                "{} draws a tile of {}x{} over {width}x{height} pixels",
                at(),
                texels[0],
                texels[1]
            ),
        );
    }
}

pub(super) fn color(inspect: &mut Inspect<'_>, position: usize, sprite: &ColorSprite) {
    let at = || format!("color source {position}");
    inspect.clip(sprite.clip, at);
    inspect.space(sprite.space, at);
    inspect.paint(sprite.paint, at);
    inspect.tile(sprite.tile, Some(PageFormat::RGBA8_PREMULTIPLIED), at);

    let flags = sprite.flags;
    let both = SpriteFlags::MASK | SpriteFlags::PAINT_ALPHA;
    if !flags.is_valid() || flags.contains(both) {
        inspect.push(
            Rule::Values,
            format!("{} has sprite flags {:#x}", at(), flags.0),
        );
    }
}
