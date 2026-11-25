//! What one pass holds for its composite: every path, each through its own clip, blended once.

mod common;

use iceblink::{ClipLinkEntry, FRAME_NO_CLIP, FrameClipShape, ROUND};

use common::raster::vello;
use iceblink_testing::vector::draw::{draw, draw_on};
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, circle, rect};

const SIDE: i32 = 128;

const CENTRES: [(f64, f64); 4] = [(32.0, 32.0), (96.0, 32.0), (32.0, 96.0), (96.0, 96.0)];

fn four_circles(scene: &mut Scene) -> Vec<u32> {
    let paint = scene.solid([255, 64, 32, 255]);
    CENTRES
        .iter()
        .map(|&(x, y)| scene.fill(circle(x, y, 20.0), paint))
        .collect()
}

#[test]
fn an_instanced_pass_draws_what_separate_passes_draw() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut instanced = Scene::new(SIDE);
    let paths = four_circles(&mut instanced);
    let planned: Vec<_> = paths.iter().map(|&path| (path, FRAME_NO_CLIP)).collect();
    instanced.pass_of(&planned, true, None);
    let mut separate = Scene::new(SIDE);
    for path in four_circles(&mut separate) {
        separate.pass(&[path]);
    }

    let one_pass = draw(&gpu, &mut vello(&gpu), &instanced);
    let four_passes = draw(&gpu, &mut vello(&gpu), &separate);

    assert_eq!(one_pass.canvas.difference(&four_passes.canvas), None);
    assert_eq!(one_pass.canvas.rgba(32, 32), [255, 64, 32, 255]);
    assert_eq!(one_pass.canvas.rgba(96, 96), [255, 64, 32, 255]);
}

#[test]
fn overlapping_paths_of_one_pass_are_blended_once() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let translucent = scene.solid([0, 0, 0, 128]);
    let left = scene.fill(rect(16.0, 48.0, 64.0, 32.0), translucent);
    let right = scene.fill(rect(48.0, 48.0, 64.0, 32.0), translucent);
    scene.pass(&[left, right]);

    let drawn = draw_on(&gpu, &mut vello(&gpu), &scene, [255; 4], 1);

    // The scratch holds the pair blended over each other, so the overlap is a quarter of white.
    let (single, overlap) = (drawn.canvas.rgba(24, 64)[0], drawn.canvas.rgba(64, 64)[0]);
    assert!(
        single.abs_diff(127) <= 3,
        "one fill over white read {single}"
    );
    assert!(overlap.abs_diff(63) <= 4, "the overlap read {overlap}");
}

#[test]
fn clipped_icons_of_one_pass_each_keep_their_own_clip() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let mut planned = Vec::new();
    for index in 0..12 {
        let (x, y) = (
            8.0 + f64::from(index % 4) * 30.0,
            8.0 + f64::from(index / 4) * 40.0,
        );
        let path = scene.fill(rect(x, y, 24.0, 24.0), white);
        let clip = scene.clip(vec![ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::RoundedRect {
                rect: [x as f32, y as f32, 24.0, 24.0],
                radii: [12.0; 8],
                shapes: ROUND,
            },
        }]);
        planned.push((path, clip));
    }
    scene.pass_of(&planned, true, None);

    let drawn = draw(&gpu, &mut vello(&gpu), &scene);

    for index in 0..12 {
        let (x, y) = (8 + (index % 4) * 30, 8 + (index / 4) * 40);
        assert_eq!(
            drawn.canvas.rgba(x + 12, y + 12),
            [255; 4],
            "icon {index} is missing"
        );
        assert_eq!(
            drawn.canvas.rgba(x + 1, y + 1),
            [0, 0, 0, 255],
            "icon {index} is unclipped"
        );
    }
}

#[test]
fn eight_passes_keep_eight_different_pictures() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    for index in 0..8_u8 {
        let paint = scene.solid([32 * index, 255 - 32 * index, 128, 255]);
        let x = f64::from(index % 4) * 32.0 + 4.0;
        let y = f64::from(index / 4) * 64.0 + 4.0;
        let path = scene.fill(rect(x, y, 24.0, 56.0), paint);
        scene.pass(&[path]);
    }

    let drawn = draw(&gpu, &mut vello(&gpu), &scene);

    for index in 0..8_u8 {
        let (x, y) = (
            i32::from(index % 4) * 32 + 16,
            i32::from(index / 4) * 64 + 32,
        );
        assert_eq!(
            drawn.canvas.rgba(x, y),
            [32 * index, 255 - 32 * index, 128, 255],
            "pass {index}"
        );
    }
}
