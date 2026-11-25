//! What the scratch costs, when it gives memory back, and what a cut pass leaves alone.

mod common;

use iceblink::FRAME_NO_CLIP;
use iceblink::geom::{Point, Rect, Size};
use iceblink_wgpu::vector::{Capacity, Extent, VectorRaster};

use common::raster::{Which, raster, vello};
use iceblink_testing::vector::draw::draw;
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, circle, rect};

const SIDE: i32 = 640;

/// One bin per layer, so the depth follows the pass count.
const ONE_BIN: Capacity = Capacity {
    side: 32,
    layers: 64,
};

/// Returns a scene of `deep` stacked passes and one pass reaching the far corner.
fn wide(deep: u32) -> Scene {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    for _ in 0..deep {
        let path = scene.fill(rect(16.0, 16.0, 24.0, 24.0), white);
        scene.pass(&[path]);
    }
    let half = f64::from(SIDE) / 2.0;
    let far = scene.fill(rect(half, half, half - 16.0, half - 16.0), white);
    scene.pass(&[far]);
    scene
}

/// Returns a scene of one small pass in the corner.
fn narrow() -> Scene {
    let mut scene = Scene::new(SIDE);
    let white = scene.solid([255; 4]);
    let path = scene.fill(rect(0.0, 0.0, 8.0, 8.0), white);
    scene.pass(&[path]);
    scene
}

#[test]
fn the_scratch_shrinks_when_the_content_does() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu).with_capacity(ONE_BIN);
    let floor = (Extent::SIZE_CLASS, Extent::SIZE_CLASS);

    raster.plan(&wide(6).plan);
    let (grown, layers, cost) = (raster.extent(), raster.layers(), raster.memory().vector);
    raster.plan(&narrow().plan);
    let freed = raster.release_idle_resources();
    raster.plan(&narrow().plan);

    assert!(
        grown > floor && layers > 1,
        "the fixture reaches further than the narrow one"
    );
    assert!(freed > 0);
    assert_eq!((raster.extent(), raster.layers()), (floor, 1));
    assert!(raster.memory().vector < cost);
}

#[test]
fn the_scratch_does_not_shrink_inside_a_fling() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu).with_capacity(ONE_BIN);
    let (large, small) = (wide(6), narrow());
    raster.plan(&large.plan);
    let held = (raster.extent(), raster.layers());

    for frame in 0..600 {
        let scene = if frame % 30 == 0 { &large } else { &small };
        raster.plan(&scene.plan);
        assert_eq!(
            (raster.extent(), raster.layers()),
            held,
            "reallocated at frame {frame}"
        );
    }
}

#[test]
fn a_cut_pass_leaves_the_pass_beside_it_alone() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(128);
    let white = scene.solid([255; 4]);
    let tall = scene.fill(rect(0.0, 0.0, 40.0, 128.0), white);
    let disc = scene.fill(circle(80.0, 32.0, 16.0), white);
    let strip = Rect::new(Point::new(0, 0), Size::new(96, 48));
    scene.pass_of(&[(tall, FRAME_NO_CLIP)], false, Some(strip));
    scene.pass(&[disc]);
    assert!(scene.plan.passes[0].clamped && !scene.plan.passes[1].clamped);

    for which in [Which::Vello, Which::Coverage] {
        let mut raster = raster(&gpu, which, 128, Capacity::DEFAULT);
        let drawn = draw(&gpu, raster.as_mut(), &scene);

        let (cut, beside) = (&drawn.plan.passes[0], &drawn.plan.passes[1]);
        assert_eq!(
            cut.target, beside.target,
            "{which:?}: the passes share a layer"
        );
        assert_eq!(
            drawn.canvas.rgba(20, 20),
            [255; 4],
            "{which:?}: the cut drawing"
        );
        assert_eq!(drawn.canvas.rgba(80, 32), [255; 4], "{which:?}: the disc");
        assert_eq!(
            drawn.canvas.rgba(65, 17),
            [0, 0, 0, 255],
            "{which:?}: the cut pass drew into the bin beside it"
        );
    }
}
