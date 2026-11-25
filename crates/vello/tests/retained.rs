//! What is kept between frames, and what is encoded again.

mod common;

use std::sync::Arc;

use iceblink::{FrameGradient, FramePaint, FrameStop, GradientShape, StopEncoding};
use kurbo::Affine;

use common::raster::vello;
use iceblink_testing::vector::draw::draw;
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, circle, rect};

const SIDE: i32 = 128;

/// Returns one frame of a disc at `offset`, drawn from `path`.
fn disc_at(path: &Arc<kurbo::BezPath>, offset: (f64, f64)) -> Scene {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let disc = scene.fill(Arc::clone(path), white);
    let disc = scene.transformed(disc, Affine::translate(offset));
    scene.pass(&[disc]);
    scene
}

#[test]
fn an_unchanged_path_is_placed_again_without_encoding() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu);
    let path = circle(32.0, 32.0, 16.0);

    draw(&gpu, &mut raster, &disc_at(&path, (0.0, 0.0)));
    let (_, (_, misses)) = raster.cache();
    let drawn = draw(&gpu, &mut raster, &disc_at(&path, (0.0, 0.0)));

    let (held, (hits, misses_after)) = raster.cache();
    assert_eq!((held, misses_after), (1, misses));
    assert!(hits >= 1);
    assert_eq!(drawn.canvas.rgba(32, 32), [255; 4]);
}

#[test]
fn a_moved_path_is_drawn_where_it_moved_without_encoding() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu);
    let path = circle(32.0, 32.0, 16.0);

    draw(&gpu, &mut raster, &disc_at(&path, (0.0, 0.0)));
    let (_, (_, misses)) = raster.cache();
    let moved = draw(&gpu, &mut raster, &disc_at(&path, (48.0, 40.0)));

    assert_eq!(raster.cache().1.1, misses);
    assert_eq!(moved.canvas.rgba(80, 72), [255; 4]);
    assert_eq!(moved.canvas.rgba(32, 32), [0, 0, 0, 255]);
}

#[test]
fn a_new_path_of_the_same_shape_is_encoded_again() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu);

    draw(
        &gpu,
        &mut raster,
        &disc_at(&circle(32.0, 32.0, 16.0), (0.0, 0.0)),
    );
    let (_, (_, misses)) = raster.cache();
    draw(
        &gpu,
        &mut raster,
        &disc_at(&circle(32.0, 32.0, 24.0), (0.0, 0.0)),
    );

    assert_eq!(
        raster.cache().1.1,
        misses + 1,
        "the cache answered for a dropped path"
    );
}

#[test]
fn one_path_drawn_twice_in_a_frame_is_encoded_once() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu);
    let path = circle(16.0, 16.0, 12.0);
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let one = scene.fill(Arc::clone(&path), white);
    let two = scene.fill(path, white);
    let two = scene.transformed(two, Affine::translate((64.0, 64.0)));
    scene.pass(&[one, two]);

    let drawn = draw(&gpu, &mut raster, &scene);

    assert_eq!(raster.cache(), (1, (1, 1)));
    assert_eq!(drawn.canvas.rgba(16, 16), [255; 4]);
    assert_eq!(drawn.canvas.rgba(80, 80), [255; 4]);
}

#[test]
fn a_restyled_stroke_is_encoded_again() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu);
    let line = Arc::new(kurbo::BezPath::from_vec(vec![
        kurbo::PathEl::MoveTo((16.0, 64.0).into()),
        kurbo::PathEl::LineTo((112.0, 64.0).into()),
    ]));
    let stroked = |style: kurbo::Stroke| {
        let mut scene = Scene::new(SIDE);
        let white = scene.solid([255; 4]);
        let path = scene.stroke(Arc::clone(&line), white, style);
        scene.pass(&[path]);
        scene
    };

    let butt = kurbo::Stroke::new(8.0).with_caps(kurbo::Cap::Butt);
    let solid = draw(&gpu, &mut raster, &stroked(butt.clone()));
    let dashed = draw(
        &gpu,
        &mut raster,
        &stroked(butt.with_dashes(0.0, [8.0, 8.0])),
    );

    assert_eq!(raster.cache().1.1, 2);
    assert_eq!(solid.canvas.rgba(28, 64), [255; 4]);
    assert_eq!(
        dashed.canvas.rgba(28, 64),
        [0, 0, 0, 255],
        "the gap of the first dash"
    );
}

#[test]
fn a_moved_gradient_moves_with_its_path() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu);
    let ramp = |scene: &mut Scene| {
        scene.paint(FramePaint::Gradient(FrameGradient {
            shape: GradientShape::Linear {
                start: [8.0, 0.0],
                end: [56.0, 0.0],
            },
            encoding: StopEncoding::SRGB,
            repeat: false,
            stops: Arc::new([
                FrameStop {
                    color: [1.0, 0.0, 0.0, 1.0],
                    offset: 0.0,
                },
                FrameStop {
                    color: [0.0, 0.0, 1.0, 1.0],
                    offset: 1.0,
                },
            ]),
        }))
    };
    let square = rect(8.0, 8.0, 48.0, 48.0);
    let frame = |offset: (f64, f64)| {
        let mut scene = Scene::new(SIDE);
        let paint = ramp(&mut scene);
        let path = scene.fill(Arc::clone(&square), paint);
        let path = scene.transformed(path, Affine::translate(offset));
        scene.pass(&[path]);
        scene
    };

    let here = draw(&gpu, &mut raster, &frame((0.0, 0.0)));
    let there = draw(&gpu, &mut raster, &frame((64.0, 64.0)));

    for x in 8..56 {
        let (one, two) = (here.canvas.rgba(x, 32), there.canvas.rgba(x + 64, 96));
        assert_eq!(
            one, two,
            "the ramp at column {x} did not move with the path"
        );
    }
}
