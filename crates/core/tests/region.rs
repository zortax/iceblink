//! Whole-pixel rounding and the tile grid of pass regions.

mod common;

use iceblink::geom::{Device, DevicePx, Point, Rect, Size};
use iceblink_core::plan::covering;
use iceblink_core::plan::pass::region::{TILE, aligned};

use crate::common::plan::rect;

/// Returns a device rectangle with fractional coordinates.
fn fractional(x: f32, y: f32, width: f32, height: f32) -> Rect<DevicePx, Device> {
    Rect::new(
        Point::new(DevicePx(x), DevicePx(y)),
        Size::new(DevicePx(width), DevicePx(height)),
    )
}

#[test]
fn covering_rounds_outwards_on_every_edge() {
    assert_eq!(covering(fractional(1.2, 2.7, 3.1, 4.1)), rect(1, 2, 4, 5));
}

#[test]
fn alignment_grows_to_the_tile_grid_and_never_shrinks() {
    let region = aligned(rect(20, 20, 10, 10), [1920, 1080]);
    assert_eq!(region, rect(16, 16, TILE, TILE));
    assert!(region.contains_rect(rect(20, 20, 10, 10)));
}

#[test]
fn a_region_outside_the_output_is_empty() {
    assert!(aligned(rect(4000, 20, 10, 10), [1920, 1080]).is_empty());
}

#[test]
fn a_region_is_cut_to_the_output() {
    let region = aligned(rect(1900, 1070, 100, 100), [1920, 1080]);
    assert_eq!(region.right(), 1920);
    assert_eq!(region.bottom(), 1080);
}
