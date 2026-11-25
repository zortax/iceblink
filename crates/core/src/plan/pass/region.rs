//! The whole-pixel regions vector passes rasterise.

use iceblink::DeviceRect;
use iceblink::geom::{Point, Size};

/// The grid pass regions are aligned to, in device pixels.
// Rasterisers work in tiles of sixteen, and a region off the grid makes every tile straddle two.
pub const TILE: i32 = 16;

/// Returns the smallest rectangle on the tile grid that contains `rect`, cut to an output of
/// `size`.
///
/// The result is empty when `rect` lies outside the output.
pub fn aligned(rect: DeviceRect, size: [u32; 2]) -> DeviceRect {
    let width = size[0].min(i32::MAX as u32) as i32;
    let height = size[1].min(i32::MAX as u32) as i32;
    let left = align_down(rect.left()).clamp(0, width);
    let top = align_down(rect.top()).clamp(0, height);
    let right = align_up(rect.right()).clamp(0, width);
    let bottom = align_up(rect.bottom()).clamp(0, height);
    DeviceRect::new(
        Point::new(left, top),
        Size::new((right - left).max(0), (bottom - top).max(0)),
    )
}

/// Returns `value` rounded down to a multiple of [`TILE`].
fn align_down(value: i32) -> i32 {
    value.div_euclid(TILE) * TILE
}

/// Returns `value` rounded up to a multiple of [`TILE`].
fn align_up(value: i32) -> i32 {
    align_down(value.saturating_add(TILE - 1))
}
