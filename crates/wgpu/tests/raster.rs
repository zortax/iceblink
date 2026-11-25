//! The coverage rasteriser on a real device.

use std::sync::Arc;

use iceblink::{
    AtlasTile, ClipLinkEntry, FrameClipShape, FrameGradient, FramePaint, FrameStop, GradientShape,
    MaskMode, Matrix4, ROUND, StopEncoding,
};
use iceblink_testing::gpu::TestDevice;
use iceblink_testing::vector::draw::draw;
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, circle, rect};
use iceblink_wgpu::vector::{VectorError, VectorRaster};
use iceblink_wgpu::vector_coverage::CoverageRaster;

const SIDE: i32 = 128;

fn raster(gpu: &TestDevice) -> CoverageRaster {
    CoverageRaster::new(&gpu.device, &gpu.queue, SIDE as u32, SIDE as u32)
}

fn rounded(space: u32, rect: [f32; 4], radius: f32) -> ClipLinkEntry {
    ClipLinkEntry {
        space,
        shape: FrameClipShape::RoundedRect {
            rect,
            radii: [radius; 8],
            shapes: ROUND,
        },
    }
}

#[test]
fn the_pattern_comes_back_through_the_scratch() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let colours = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ];
    let mut paths = Vec::new();
    for (index, colour) in colours.into_iter().enumerate() {
        let (x, y) = ((index % 2) as f64 * 64.0, (index / 2) as f64 * 64.0);
        let paint = scene.solid(colour);
        paths.push(scene.fill(rect(x, y, 64.0, 64.0), paint));
    }
    scene.pass(&paths);

    let drawn = draw(&gpu, &mut raster(&gpu), &scene);

    drawn.outcome.expect("the frame rasterises");
    for (index, colour) in colours.into_iter().enumerate() {
        let (x, y) = ((index % 2) as i32 * 64 + 32, (index / 2) as i32 * 64 + 32);
        assert_eq!(drawn.canvas.rgba(x, y), colour, "quadrant {index}");
    }
}

#[test]
fn a_residual_clip_is_applied_inside_the_scratch() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(16.0, 16.0, 96.0, 96.0), paint);
    let residual = scene.clip(vec![rounded(0, [16.0, 16.0, 96.0, 96.0], 48.0)]);
    scene.pass_of(&[(path, residual)], false, None);

    let mut raster = raster(&gpu);
    let drawn = draw(&gpu, &mut raster, &scene);

    drawn.outcome.expect("the frame rasterises");
    assert_eq!(
        drawn.canvas.rgba(64, 64),
        [255; 4],
        "the middle of the disc"
    );
    assert_eq!(
        drawn.canvas.rgba(20, 20),
        [0, 0, 0, 255],
        "outside the disc"
    );
    assert!(raster.last_frame().clip_layers >= 1);
}

#[test]
fn a_residual_measured_in_a_moved_space_moves_with_it() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(0.0, 0.0, 128.0, 128.0), paint);
    let space = scene.space(Matrix4::translation(64.0, 0.0, 0.0));
    let residual = scene.clip(vec![rounded(space, [0.0, 0.0, 64.0, 128.0], 0.0)]);
    scene.pass_of(&[(path, residual)], false, None);

    let drawn = draw(&gpu, &mut raster(&gpu), &scene);

    assert_eq!(drawn.canvas.rgba(32, 64), [0, 0, 0, 255]);
    assert_eq!(drawn.canvas.rgba(96, 64), [255; 4]);
}

#[test]
fn a_path_whose_residual_holds_a_mask_is_left_out_and_counted() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(16.0, 16.0, 32.0, 32.0), paint);
    let residual = scene.clip(vec![ClipLinkEntry {
        space: 0,
        shape: FrameClipShape::Mask {
            rect: [0.0, 0.0, 64.0, 64.0],
            tile: AtlasTile::default(),
            mode: MaskMode::ALPHA,
        },
    }]);
    scene.pass_of(&[(path, residual)], false, None);

    let mut raster = raster(&gpu);
    let drawn = draw(&gpu, &mut raster, &scene);

    drawn.outcome.expect("an unclippable path is no failure");
    assert_eq!(drawn.canvas.rgba(32, 32), [0, 0, 0, 255]);
    assert_eq!(raster.last_frame().unclippable, 1);
}

#[test]
fn a_gradient_is_filled_with_the_mean_colour_of_its_ramp_as_the_rasteriser_declares() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.paint(FramePaint::Gradient(FrameGradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [128.0, 0.0],
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
    }));
    let path = scene.fill(circle(64.0, 64.0, 40.0), paint);
    scene.pass(&[path]);

    let mut raster = raster(&gpu);
    let drawn = draw(&gpu, &mut raster, &scene);

    let [red, green, blue, alpha] = drawn.canvas.rgba(64, 64);
    assert!(red.abs_diff(128) <= 1 && green == 0 && blue.abs_diff(128) <= 1 && alpha == 255);
    assert!(raster.flat_gradients());
}

#[test]
fn a_path_that_paints_nothing_is_counted() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.paint(FramePaint::None);
    let path = scene.fill(rect(16.0, 16.0, 32.0, 32.0), paint);
    scene.pass(&[path]);

    let mut raster = raster(&gpu);
    let drawn = draw(&gpu, &mut raster, &scene);

    assert_eq!(drawn.canvas.rgba(32, 32), [0, 0, 0, 255]);
    assert_eq!(raster.last_frame().unpaintable, 1);
}

#[test]
fn the_scratch_is_two_textures_and_no_fixed_memory() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(0.0, 0.0, 100.0, 50.0), paint);
    scene.pass(&[path]);
    let mut raster = raster(&gpu);

    draw(&gpu, &mut raster, &scene);

    let (width, height) = raster.extent();
    let one = u64::from(width) * u64::from(height) * u64::from(raster.layers()) * 4;
    assert_eq!(raster.scratch_bytes(), 2 * one);
    let memory = raster.memory();
    assert_eq!(memory.fixed, 0);
    assert!(memory.vector >= raster.scratch_bytes());
}

#[test]
fn idle_release_frees_the_scratch_and_the_next_frame_draws_again() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(16.0, 16.0, 32.0, 32.0), paint);
    scene.pass(&[path]);
    let mut raster = raster(&gpu);
    draw(&gpu, &mut raster, &scene);

    let freed = raster.release_idle_resources();
    let drawn = draw(&gpu, &mut raster, &scene);

    assert!(freed > 0);
    assert_eq!(drawn.canvas.rgba(32, 32), [255; 4]);
}

#[test]
fn tables_past_what_a_texture_holds_are_reported_as_out_of_capacity() {
    let Some(gpu) = open_device() else {
        return;
    };
    let limit = gpu.device.limits().max_texture_dimension_2d as usize * 256;
    // Every fill of a path 4094 pixels tall takes 4096 bands.
    let fills = limit / 4096 + 1;
    let side = 4112;
    let mut scene = Scene::new(side);
    let paint = scene.solid([255, 255, 255, 255]);
    let first = scene.fill(rect(0.0, 0.0, 8.0, 8.0), paint);
    scene.pass(&[first]);
    let tall: Vec<u32> = (0..fills)
        .map(|_| scene.fill(rect(0.0, 0.0, 8.0, 4094.0), paint))
        .collect();
    scene.pass(&tall);
    let mut raster = CoverageRaster::new(&gpu.device, &gpu.queue, 16, 16);

    let plan = raster.plan(&scene.plan);
    raster.clear_targets(&plan, 0);
    let outcome = raster.prepare(&mut scene.frame(&plan, 0, &gpu));

    assert_eq!(plan.rounds(), 1);
    assert!(
        matches!(outcome, Err(VectorError::OutOfCapacity { prepared: 1, .. })),
        "{outcome:?}"
    );
}
