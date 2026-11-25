//! Path coverage against the exact area coverage of each pixel.
//!
//! The exact picture is measured on the host by sampling every pixel on a 16 by 16 grid. Both
//! pictures agree on every interior pixel, and their edges agree within limits.

mod common;

use std::sync::Arc;

use iceblink::{ClipLinkEntry, FrameClipShape, ROUND};
use iceblink_wgpu::vector::VectorRaster;
use kurbo::{Affine, BezPath, Shape as _};

use common::raster::{coverage, vello};
use iceblink_testing::vector::canvas::Canvas;
use iceblink_testing::vector::draw::draw;
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, circle, rect};

const SIDE: i32 = 128;

/// The largest mean and the largest single difference over edge pixels, in levels of 255.
#[derive(Clone, Copy, Debug)]
struct Limits {
    mean: f64,
    worst: u8,
}

/// Vello on straight edges, which it measures exactly.
// The reference's own sampling error at a corner reaches about 8.
const STRAIGHT: Limits = Limits {
    mean: 2.5,
    worst: 16,
};

/// Vello on curves.
// Vello flattens curves coarsely: on an RTX 3080 Ti and on lavapipe the edges of these circles
// differ by 14 on average and by up to 73 at one pixel.
const CURVED: Limits = Limits {
    mean: 18.0,
    worst: 80,
};

/// The coverage rasteriser, whose sixteen samples resolve an edge to a seventeenth.
// Measured at a mean of 6.4 and at most 54 on both devices.
const SAMPLED: Limits = Limits {
    mean: 8.0,
    worst: 64,
};

/// White shapes on black: outlines in device space, each cut to an optional device rectangle.
struct Shapes(Vec<(BezPath, Option<kurbo::Rect>)>);

impl Shapes {
    /// The exact picture.
    fn exact(&self) -> Canvas {
        let mut canvas = Canvas::filled(SIDE, [0, 0, 0, 255]);
        for (outline, cut) in &self.0 {
            let bounds = outline.bounding_box();
            let (x0, y0) = (
                bounds.x0.floor().max(0.0) as i32,
                bounds.y0.floor().max(0.0) as i32,
            );
            let (x1, y1) = (
                bounds.x1.ceil().min(f64::from(SIDE)) as i32,
                bounds.y1.ceil().min(f64::from(SIDE)) as i32,
            );
            for y in y0..y1 {
                for x in x0..x1 {
                    let mut inside = 0_u32;
                    for sample in 0..256 {
                        let point = kurbo::Point::new(
                            f64::from(x) + (f64::from(sample % 16) + 0.5) / 16.0,
                            f64::from(y) + (f64::from(sample / 16) + 0.5) / 16.0,
                        );
                        let cut_away = cut.is_some_and(|cut| !cut.contains(point));
                        inside += u32::from(!cut_away && outline.winding(point) != 0);
                    }
                    let alpha = (inside as f32 / 256.0 * 255.0).round() as u8;
                    canvas.blend(x, y, [255, 255, 255, alpha]);
                }
            }
        }
        canvas
    }
}

/// Draws `scene` through both rasterisers and compares each with `shapes`, holding vello to
/// `limits`.
fn check(scene: &Scene, shapes: &Shapes, label: &str, limits: Limits) {
    let Some(gpu) = open_device() else {
        return;
    };
    let exact = shapes.exact();
    for (name, mut raster, limits) in [
        (
            "vello",
            Box::new(vello(&gpu)) as Box<dyn VectorRaster>,
            limits,
        ),
        ("coverage", Box::new(coverage(&gpu, SIDE)), SAMPLED),
    ] {
        let drawn = draw(&gpu, raster.as_mut(), scene);
        let found = drawn.canvas.agreement(&exact, [0, 0, 0, 255]);
        println!("{label} through {name}: {found:?}");
        assert!(
            found.edge >= 100,
            "{label}: the shapes cover almost nothing"
        );
        assert!(
            found.interior_worst <= 1,
            "{label} through {name}: {found:?}"
        );
        assert!(
            found.mean <= limits.mean && found.worst <= limits.worst,
            "{label} through {name}: {found:?} past {limits:?}"
        );
    }
}

/// Returns one pass of white fills of `outlines`, and the shapes they stand for.
fn fills(outlines: &[Arc<BezPath>]) -> (Scene, Shapes) {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let paths: Vec<u32> = outlines
        .iter()
        .map(|outline| scene.fill(Arc::clone(outline), white))
        .collect();
    scene.pass(&paths);
    let shapes = outlines
        .iter()
        .map(|outline| (BezPath::clone(outline), None))
        .collect();
    (scene, Shapes(shapes))
}

#[test]
fn circles_match_their_exact_coverage() {
    let (scene, shapes) = fills(&[
        circle(24.0, 24.0, 16.0),
        circle(80.3, 30.6, 21.7),
        circle(40.5, 92.5, 6.25),
        circle(96.0, 96.0, 24.0),
    ]);

    check(&scene, &shapes, "circles", CURVED);
}

#[test]
fn rectangles_off_the_pixel_grid_match_their_exact_coverage() {
    let (scene, shapes) = fills(&[
        rect(8.0, 8.0, 40.0, 24.0),
        rect(60.25, 10.5, 30.5, 20.75),
        rect(10.4, 60.6, 50.3, 40.1),
    ]);

    check(&scene, &shapes, "rectangles", STRAIGHT);
}

#[test]
fn strokes_match_their_exact_coverage() {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let ring = circle(64.0, 64.0, 40.0);
    let style = kurbo::Stroke::new(5.0);
    let path = scene.stroke(Arc::clone(&ring), white, style.clone());
    scene.pass(&[path]);
    let outline = kurbo::stroke(ring.iter(), &style, &kurbo::StrokeOpts::default(), 0.01);

    check(&scene, &Shapes(vec![(outline, None)]), "a ring", CURVED);
}

#[test]
fn shapes_under_a_scale_match_their_exact_coverage() {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let small = circle(20.0, 20.0, 12.0);
    let scale = Affine::scale_non_uniform(2.5, 1.75);
    let path = scene.fill(Arc::clone(&small), white);
    let path = scene.transformed(path, scale);
    scene.pass(&[path]);

    check(
        &scene,
        &Shapes(vec![(scale * BezPath::clone(&small), None)]),
        "a scaled circle",
        CURVED,
    );
}

#[test]
fn a_whole_pixel_residual_clip_matches_its_exact_coverage() {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let disc = circle(64.0, 64.0, 40.0);
    let path = scene.fill(Arc::clone(&disc), white);
    let clip = scene.clip(vec![ClipLinkEntry {
        space: 0,
        shape: FrameClipShape::RoundedRect {
            rect: [40.0, 30.0, 60.0, 50.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }]);
    scene.pass_of(&[(path, clip)], false, None);
    let cut = kurbo::Rect::new(40.0, 30.0, 100.0, 80.0);

    check(
        &scene,
        &Shapes(vec![(BezPath::clone(&disc), Some(cut))]),
        "a clipped disc",
        CURVED,
    );
}
