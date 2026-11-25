//! Opaque covers: the quads a renderer may start a draw rectangle from.

mod common;

use iceblink::DeviceRect;
use iceblink::damage::DamageSet;
use iceblink::geom::{Device, DevicePx, Point, Rect, Size};
use iceblink_core::plan::inner_pixels;

use crate::common::plan::scene::Scene;
use crate::common::plan::{invariants, rect};

/// Pushes an opaque quad over `ink`, covering its inner pixels.
fn opaque(scene: &mut Scene, ink: DeviceRect) {
    let bounds: Rect<DevicePx, Device> = Rect::new(
        Point::new(DevicePx(ink.left() as f32), DevicePx(ink.top() as f32)),
        Size::new(DevicePx(ink.width() as f32), DevicePx(ink.height() as f32)),
    );
    scene.cover(ink, inner_pixels(bounds));
}

/// Plans `scene` whole, checks it, and returns its cover rectangles.
fn covers(scene: &mut Scene) -> Vec<DeviceRect> {
    let planned = scene.plan(&DamageSet::full());
    invariants::check(scene, &DamageSet::full(), &planned);
    planned
        .frame
        .opaque_covers
        .iter()
        .map(|cover| cover.rect)
        .collect()
}

/// Returns the whole output less the pixel each edge softens.
fn inset() -> DeviceRect {
    rect(1, 1, 98, 98)
}

#[test]
fn the_inner_pixels_of_fractional_bounds_are_rounded_inwards_and_inset() {
    let bounds: Rect<DevicePx, Device> = Rect::new(
        Point::new(DevicePx(0.5), DevicePx(2.0)),
        Size::new(DevicePx(10.0), DevicePx(10.25)),
    );
    assert_eq!(inner_pixels(bounds), rect(2, 3, 7, 8));
}

#[test]
fn an_opaque_quad_covers_its_inner_pixels() {
    let mut scene = Scene::sized([100, 100]);
    opaque(&mut scene, rect(0, 0, 100, 100));
    assert_eq!(covers(&mut scene), vec![inset()]);
}

#[test]
fn a_quad_pushed_without_a_cover_covers_nothing() {
    let mut scene = Scene::sized([100, 100]);
    scene.quad(rect(0, 0, 100, 100));
    assert!(covers(&mut scene).is_empty());
}

#[test]
fn a_later_quad_over_the_same_pixels_replaces_the_one_before_it() {
    let mut scene = Scene::sized([100, 100]);
    opaque(&mut scene, rect(0, 0, 100, 100));
    opaque(&mut scene, rect(0, 0, 100, 100));
    let planned = scene.plan(&DamageSet::full());
    let held = &planned.frame.opaque_covers;
    assert_eq!(held.len(), 1);
    assert_eq!(
        held[0].at, 1,
        "the later one is kept, so a renderer skips more"
    );
    assert_eq!(held[0].batch, 0);
}

#[test]
fn a_quad_inside_another_is_kept_beside_it() {
    let mut scene = Scene::sized([100, 100]);
    opaque(&mut scene, rect(0, 0, 100, 100));
    opaque(&mut scene, rect(20, 20, 40, 40));
    assert_eq!(covers(&mut scene), vec![inset(), rect(21, 21, 38, 38)]);
}

#[test]
fn nothing_kept_contains_anything_else_kept() {
    let mut scene = Scene::sized([100, 100]);
    for step in (0..6).rev() {
        let inset = step * 4;
        opaque(
            &mut scene,
            rect(inset, inset, 100 - inset * 2, 100 - inset * 2),
        );
    }
    assert_eq!(covers(&mut scene), vec![inset()]);
}

#[test]
fn the_largest_four_are_kept() {
    let mut scene = Scene::sized([400, 100]);
    for (x, side) in [(0, 20), (40, 30), (80, 12), (120, 40), (200, 50), (300, 8)] {
        opaque(&mut scene, rect(x, 0, side, side));
    }
    let mut sides: Vec<i32> = covers(&mut scene)
        .iter()
        .map(|cover| cover.width() + 2)
        .collect();
    sides.sort_unstable();
    assert_eq!(sides, vec![20, 30, 40, 50]);
}

#[test]
fn a_quad_after_a_group_covers_nothing() {
    let mut scene = Scene::sized([100, 100]);
    scene.begin_group(rect(0, 0, 10, 10));
    scene.quad(rect(0, 0, 10, 10));
    scene.end_group();
    opaque(&mut scene, rect(0, 0, 100, 100));
    assert!(covers(&mut scene).is_empty());
}

#[test]
fn a_backdrop_allows_no_cover() {
    let mut scene = Scene::sized([100, 100]);
    opaque(&mut scene, rect(0, 0, 100, 100));
    scene.backdrop(rect(0, 0, 50, 50), true);
    assert!(covers(&mut scene).is_empty());
}
