// DERIVED-FROM: the GPUI project, crates/gpui_wgpu/src/shaders.wgsl and
// crates/gpui_wgpu/src/shaders_subpixel.wgsl (Apache-2.0)
// Ported from iceblink-wgsl's `sprite.wgsl`, `sprite_mono.wgsl`, `sprite_color.wgsl` and
// `subpixel.wgsl`. The sprite layout, the unit-square expansion, the tile lookup and the
// per-channel blend are adapted from that work, which is licensed under the Apache License, Version
// 2.0, and have been modified: a tile is addressed in texels of its atlas page, sprites are filled
// with a paint, colour sprites repeat, tint masks and take a paint's alpha, and the rounded frame
// has elliptical semi-axes and an exponent per corner.

//! Sprites: coverage tiles filled with a paint, and full-colour tiles.

mod color;
mod mono;
mod subpixel;

use iceblink::AtlasTile;

use crate::pages::{Page, nearest, trilinear};

pub(crate) use self::color::draw_color_sprites;
pub(crate) use self::mono::draw_mono_sprites;
pub(crate) use self::subpixel::draw_subpixel_sprites;

/// Returns the tile's rectangle as `[x, y, width, height]` in texels.
fn tile_rect(tile: &AtlasTile) -> [f32; 4] {
    tile.rect.map(|value| value as f32)
}

/// Returns `position` clamped to the tile's inner half-texel, so a filter never reads a
/// neighbouring tile.
fn clamped(position: [f32; 2], rect: [f32; 4]) -> [f32; 2] {
    let inset = [(rect[2] * 0.5).min(0.5), (rect[3] * 0.5).min(0.5)];
    [
        position[0].clamp(rect[0] + inset[0], rect[0] + rect[2] - inset[0]),
        position[1].clamp(rect[1] + inset[1], rect[1] + rect[3] - inset[1]),
    ]
}

/// Returns a coverage page at `position`, read as its nearest texel.
fn sample_coverage(page: &Page, position: [f32; 2], rect: [f32; 4]) -> [f32; 4] {
    nearest(page, clamped(position, rect))
}

/// Returns a colour page at `position` and level of detail `lod`.
fn sample_color(page: &Page, position: [f32; 2], rect: [f32; 4], lod: f32) -> [f32; 4] {
    trilinear(page, clamped(position, rect), lod)
}

/// Returns where a local point lies in the tile, and how far one device pixel along x and along
/// y moves that position.
fn texel_of(
    local: [f32; 2],
    extent: [f32; 2],
    rect: [f32; 4],
    steps: [[f32; 2]; 2],
) -> ([f32; 2], [[f32; 2]; 2]) {
    let ratio = [rect[2] / extent[0], rect[3] / extent[1]];
    let position = [rect[0] + local[0] * ratio[0], rect[1] + local[1] * ratio[1]];
    let moved = steps.map(|step| [step[0] * ratio[0], step[1] * ratio[1]]);
    (position, moved)
}
