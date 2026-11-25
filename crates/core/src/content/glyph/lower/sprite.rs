//! Glyph tiles drawn as sprites.

use iceblink::{ColorSprite, PaintRef, ROUND, Sprite, SpriteFlags};

use crate::content::glyph::answers::Placed;
use crate::content::glyph::keys::Spot;
use crate::content::target::Target;

/// Returns the coverage sprite of a placed glyph.
pub(super) fn coverage_glyph(
    paint: PaintRef,
    target: &Target,
    spot: Spot,
    placed: &Placed,
) -> Sprite {
    Sprite {
        bounds: spot.bounds(placed.placement, [placed.tile.rect[2], placed.tile.rect[3]]),
        paint,
        paint_origin: target.offset,
        tile: placed.tile,
        clip: target.clip,
        space: target.space,
        _pad: 0,
    }
}

/// Returns the colour sprite of a placed colour glyph, which takes its alpha from `paint`.
pub(super) fn color_glyph(
    paint: PaintRef,
    target: &Target,
    spot: Spot,
    placed: &Placed,
) -> ColorSprite {
    let bounds = spot.bounds(placed.placement, [placed.tile.rect[2], placed.tile.rect[3]]);
    ColorSprite {
        bounds,
        frame: bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        tile: placed.tile,
        paint,
        paint_origin: target.offset,
        opacity: 1.0,
        clip: target.clip,
        space: target.space,
        flags: SpriteFlags::PAINT_ALPHA,
    }
}
