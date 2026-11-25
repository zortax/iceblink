//! Reading pages as the reference renderer's samplers do.
//!
//! Every sampler clamps to the edge of the level it reads. Positions are in texels of level 0,
//! with texel centres at half-integers.

use crate::math::Floor as _;
use crate::math::vec::mix4;
use crate::pages::Page;

/// Returns the texel of level 0 that holds `position`.
pub(crate) fn nearest(page: &Page, position: [f32; 2]) -> [f32; 4] {
    page.load(
        0,
        position[0].floored() as i32,
        position[1].floored() as i32,
    )
}

/// Returns `level` sampled bilinearly at `position`, given in texels of that level.
fn bilinear(page: &Page, level: usize, position: [f32; 2]) -> [f32; 4] {
    let at = [position[0] - 0.5, position[1] - 0.5];
    let base = [at[0].floored(), at[1].floored()];
    let weight = [at[0] - base[0], at[1] - base[1]];
    let [x, y] = [base[0] as i32, base[1] as i32];
    let low = mix4(
        page.load(level, x, y),
        page.load(level, x + 1, y),
        weight[0],
    );
    let high = mix4(
        page.load(level, x, y + 1),
        page.load(level, x + 1, y + 1),
        weight[0],
    );
    mix4(low, high, weight[1])
}

/// Returns the page sampled at `position` and level of detail `lod`, bilinearly within the two
/// nearest levels and linearly between them.
pub(crate) fn trilinear(page: &Page, position: [f32; 2], lod: f32) -> [f32; 4] {
    let [width, height] = page.size();
    let last = page.levels.len().saturating_sub(1) as f32;
    let lod = lod.clamp(0.0, last);
    let at_level = |level: usize| {
        let held = &page.levels[level];
        let position = [
            position[0] / width.max(1) as f32 * held.width as f32,
            position[1] / height.max(1) as f32 * held.height as f32,
        ];
        bilinear(page, level, position)
    };
    let below = lod.floored() as usize;
    let fraction = lod - lod.floored();
    if fraction <= 0.0 || below + 1 >= page.levels.len() {
        return at_level(below);
    }
    mix4(at_level(below), at_level(below + 1), fraction)
}
