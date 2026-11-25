//! Mono, subpixel and colour sprites.

use iceblink::{ColorSprite, PaintKind, Sprite, SpriteFlags};

use crate::capture::mirror::Mirror;
use crate::capture::text::number::{float, rect};
use crate::capture::transcript::instance::boxes::{corners, origin};
use crate::capture::transcript::instance::tile;
use crate::capture::transcript::{paint, space};

/// Renders a mono or subpixel sprite under the name `kind`.
pub(crate) fn coverage(mirror: &Mirror, kind: &str, sprite: &Sprite) -> String {
    let mut line = format!(
        "{kind} bounds={} paint={} tile={}",
        rect(sprite.bounds),
        paint::reference(mirror, sprite.paint),
        tile(sprite.tile)
    );
    origin(&mut line, sprite.paint_origin);
    line.push_str(&space::suffix(mirror, sprite.clip, sprite.space));
    line
}

pub(crate) fn color(mirror: &Mirror, sprite: &ColorSprite) -> String {
    let mut line = format!(
        "color bounds={} tile={}",
        rect(sprite.bounds),
        tile(sprite.tile)
    );
    if sprite.frame != sprite.bounds {
        line.push_str(&format!(" frame={}", rect(sprite.frame)));
    }
    if sprite.paint.kind != PaintKind::NONE {
        line.push_str(&format!(
            " paint={}",
            paint::reference(mirror, sprite.paint)
        ));
    }
    if sprite.opacity != 1.0 {
        line.push_str(&format!(" opacity={}", float(sprite.opacity)));
    }
    let names = [
        (SpriteFlags::GRAYSCALE, "grayscale"),
        (SpriteFlags::MASK, "mask"),
        (SpriteFlags::REPEAT_X, "repeat_x"),
        (SpriteFlags::REPEAT_Y, "repeat_y"),
        (SpriteFlags::NEAREST, "nearest"),
        (SpriteFlags::PAINT_ALPHA, "paint_alpha"),
    ];
    for (flag, name) in names {
        if sprite.flags.contains(flag) {
            line.push(' ');
            line.push_str(name);
        }
    }
    corners(&mut line, &sprite.radii, &sprite.shapes);
    origin(&mut line, sprite.paint_origin);
    line.push_str(&space::suffix(mirror, sprite.clip, sprite.space));
    line
}
