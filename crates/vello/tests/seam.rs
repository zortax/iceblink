//! Two rasterisers, one contract: the same frame through both on one device.

mod common;

use iceblink::{AtlasTile, ClipLinkEntry, FillRule, FrameClipShape, MaskMode, PathClip, ROUND};
use iceblink_wgpu::vector::Capacity;

use common::raster::{Which, coverage, raster, vello};
use iceblink_testing::vector::canvas::Canvas;
use iceblink_testing::vector::draw::draw;
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, circle, rect};

const SIDE: i32 = 128;

/// The largest difference at one edge pixel between the two rasterisers.
// Sixteen samples resolve an edge to a seventeenth, 16 of 255. Flattening the circle of radius
// eighteen costs more: up to 61 of 255 on one edge pixel, measured on an RTX 3080 Ti.
const WORST: u8 = 72;

fn rounded(rect: [f32; 4], radii: [f32; 8]) -> ClipLinkEntry {
    ClipLinkEntry {
        space: 0,
        shape: FrameClipShape::RoundedRect {
            rect,
            radii,
            shapes: ROUND,
        },
    }
}

/// Returns a scene of a straight edge, a curved one, and a residual clip in one pass.
fn shared_scene() -> Scene {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let square = scene.fill(rect(8.0, 8.0, 40.0, 40.0), white);
    let disc = scene.fill(circle(90.0, 28.0, 18.0), white);
    let clipped = scene.fill(rect(16.0, 64.0, 96.0, 56.0), white);
    let radii = [20.0, 10.0, 20.0, 10.0, 20.0, 10.0, 20.0, 10.0];
    let clip = scene.clip(vec![rounded([24.0, 72.0, 80.0, 40.0], radii)]);
    let none = iceblink::FRAME_NO_CLIP;
    scene.pass_of(
        &[(square, none), (disc, none), (clipped, clip)],
        false,
        None,
    );
    scene
}

/// Returns the largest difference, the pixels that differ, and the interior pixels, asserting
/// that the two agree on every interior pixel.
fn compare(one: &Canvas, two: &Canvas) -> (u8, u32, u32) {
    let (mut worst, mut differing, mut interior) = (0, 0, 0);
    for y in 0..one.side {
        for x in 0..one.side {
            let (a, b) = (one.rgba(x, y), two.rgba(x, y));
            let error = (0..4).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0);
            worst = worst.max(error);
            differing += u32::from(error > 0);
            if (a[0] == 255 && b[0] == 255) || (a[0] == 0 && b[0] == 0) {
                interior += 1;
                assert_eq!(a, b, "the two disagree at ({x}, {y}), off every edge");
            }
        }
    }
    (worst, differing, interior)
}

#[test]
fn both_rasterisers_cover_the_same_pixels_within_the_tolerance() {
    let Some(gpu) = open_device() else {
        return;
    };
    let scene = shared_scene();

    let by_vello = draw(&gpu, &mut vello(&gpu), &scene);
    let by_coverage = draw(&gpu, &mut coverage(&gpu, SIDE), &scene);

    let (worst, differing, interior) = compare(&by_vello.canvas, &by_coverage.canvas);
    println!("vello against coverage: worst {worst}, differing {differing}, interior {interior}");
    assert!(
        interior > 12_000,
        "only {interior} interior pixels were compared"
    );
    assert!(worst <= WORST, "the rasterisers disagree by {worst}");
    assert!(
        differing > 0 && differing < 500,
        "{differing} pixels differ"
    );
}

#[test]
fn a_straight_edge_lands_within_one_level_in_both_rasterisers() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    // A quarter of the left column and half of the top row lie outside.
    let path = scene.fill(rect(40.25, 40.5, 32.0, 32.0), white);
    scene.pass(&[path]);

    let by_vello = draw(&gpu, &mut vello(&gpu), &scene);
    let by_coverage = draw(&gpu, &mut coverage(&gpu, SIDE), &scene);

    for (at, expected) in [((40, 41), 191_u8), ((41, 40), 128), ((40, 40), 96)] {
        let one = by_vello.canvas.rgba(at.0, at.1)[0];
        let two = by_coverage.canvas.rgba(at.0, at.1)[0];
        assert!(
            one.abs_diff(expected) <= 1 && two.abs_diff(expected) <= 1,
            "at {at:?} the two read {one} and {two}, where the area says {expected}"
        );
    }
}

#[test]
fn vello_applies_a_residual_clip_inside_its_scratch() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let path = scene.fill(rect(0.0, 0.0, 128.0, 128.0), white);
    let clip = scene.clip(vec![rounded([32.0, 32.0, 64.0, 64.0], [32.0; 8])]);
    scene.pass_of(&[(path, clip)], false, None);

    let drawn = draw(&gpu, &mut vello(&gpu), &scene);

    assert_eq!(drawn.canvas.rgba(64, 64), [255; 4], "inside the clip");
    assert_eq!(drawn.canvas.rgba(34, 34), [0, 0, 0, 255], "the cut corner");
}

#[test]
fn vello_measures_a_residual_link_in_its_own_space() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let path = scene.fill(rect(0.0, 0.0, 128.0, 128.0), white);
    let space = scene.space(iceblink::Matrix4::translation(64.0, 0.0, 0.0));
    let clip = scene.clip(vec![ClipLinkEntry {
        space,
        shape: FrameClipShape::RoundedRect {
            rect: [0.0, 0.0, 64.0, 128.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }]);
    scene.pass_of(&[(path, clip)], false, None);

    let drawn = draw(&gpu, &mut vello(&gpu), &scene);

    assert_eq!(drawn.canvas.rgba(32, 64), [0, 0, 0, 255]);
    assert_eq!(drawn.canvas.rgba(96, 64), [255; 4]);
}

#[test]
fn an_even_odd_ring_under_a_path_clip_matches_on_both_rasterisers() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let mut rings = kurbo::BezPath::new();
    for radius in [48.0, 24.0] {
        let circle = kurbo::Circle::new((64.0, 64.0), radius);
        rings.extend(kurbo::Shape::path_elements(&circle, 0.01));
    }
    let path = scene.fill(std::sync::Arc::new(rings), white);
    let ring = &mut scene.paths[path as usize];
    ring.fill.as_mut().expect("a fill").rule = FillRule::EVEN_ODD;
    ring.clips = std::sync::Arc::new([PathClip {
        path: rect(0.0, 0.0, 64.0, 128.0),
        rule: FillRule::NON_ZERO,
    }]);
    scene.pass(&[path]);

    let by_vello = draw(&gpu, &mut vello(&gpu), &scene);
    let by_coverage = draw(&gpu, &mut coverage(&gpu, SIDE), &scene);

    for (name, drawn) in [("vello", &by_vello), ("coverage", &by_coverage)] {
        assert_eq!(drawn.canvas.rgba(28, 64), [255; 4], "{name}: the ring");
        assert_eq!(
            drawn.canvas.rgba(60, 64),
            [0, 0, 0, 255],
            "{name}: the even-odd hole"
        );
        assert_eq!(
            drawn.canvas.rgba(100, 64),
            [0, 0, 0, 255],
            "{name}: past the path clip"
        );
    }
}

#[test]
fn vello_leaves_out_and_counts_a_path_whose_residual_has_no_outline() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let masked = scene.fill(rect(16.0, 16.0, 64.0, 64.0), white);
    let plain = scene.fill(rect(96.0, 96.0, 16.0, 16.0), white);
    let mask = scene.clip(vec![ClipLinkEntry {
        space: 0,
        shape: FrameClipShape::Mask {
            rect: [0.0, 0.0, 64.0, 64.0],
            tile: AtlasTile::default(),
            mode: MaskMode::ALPHA,
        },
    }]);
    scene.pass_of(
        &[(masked, mask), (plain, iceblink::FRAME_NO_CLIP)],
        false,
        None,
    );

    let mut raster = vello(&gpu);
    let drawn = draw(&gpu, &mut raster, &scene);

    drawn.outcome.expect("an unclippable path is no failure");
    assert_eq!(raster.last_frame().unclippable, 1);
    assert_eq!(
        drawn.canvas.rgba(104, 104),
        [255; 4],
        "the plain path is still drawn"
    );
}

#[test]
fn a_stretched_stroke_matches_on_both_rasterisers() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let ring = scene.stroke(circle(0.0, 0.0, 10.0), white, kurbo::Stroke::new(2.0));
    let stretch =
        kurbo::Affine::translate((40.0, 40.0)) * kurbo::Affine::scale_non_uniform(2.0, 1.0);
    let path = scene.transformed(ring, stretch);
    scene.pass(&[path]);

    let by_vello = draw(&gpu, &mut vello(&gpu), &scene);
    let by_coverage = draw(&gpu, &mut coverage(&gpu, SIDE), &scene);

    // The pixels the band covers to at least half, along each centre line.
    let across = |canvas: &Canvas| (0..40).filter(|&x| canvas.rgba(x, 40)[0] >= 128).count();
    let down = |canvas: &Canvas| (0..40).filter(|&y| canvas.rgba(40, y)[0] >= 128).count();
    for (name, drawn) in [("vello", &by_vello), ("coverage", &by_coverage)] {
        assert!(
            across(&drawn.canvas).abs_diff(4) <= 1,
            "{name}: the stretched side"
        );
        assert!(
            down(&drawn.canvas).abs_diff(2) <= 1,
            "{name}: the unstretched side"
        );
    }
    let (worst, _, _) = compare(&by_vello.canvas, &by_coverage.canvas);
    assert!(worst <= WORST, "the rasterisers disagree by {worst}");
}

#[test]
fn a_pass_past_the_edge_of_the_output_stays_in_its_bin() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let red = scene.solid([255, 0, 0, 255]);
    let green = scene.solid([0, 255, 0, 255]);
    let over = scene.fill(rect(16.0, 100.0, 32.0, 60.0), red);
    let one = scene.fill(rect(8.0, 84.0, 8.0, 8.0), green);
    let two = scene.fill(rect(10.0, 84.0, 8.0, 8.0), green);
    scene.pass(&[over]).pass(&[one, two]);
    assert!(scene.plan.passes[0].overhangs);

    for which in [Which::Vello, Which::Coverage] {
        let drawn = draw(
            &gpu,
            raster(&gpu, which, SIDE, Capacity::DEFAULT).as_mut(),
            &scene,
        );

        assert_eq!(
            drawn.canvas.rgba(12, 88),
            [0, 255, 0, 255],
            "{which:?}: the green squares"
        );
        for (x, y) in [(2, 80), (2, 90), (24, 82)] {
            assert_eq!(
                drawn.canvas.rgba(x, y),
                [0, 0, 0, 255],
                "{which:?}: the red square's ink reached the green pass at ({x}, {y})"
            );
        }
        assert_eq!(
            drawn.canvas.rgba(32, 120),
            [255, 0, 0, 255],
            "{which:?}: the red square"
        );
    }
}
