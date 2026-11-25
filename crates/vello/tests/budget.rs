//! What a rasteriser holds, what it shares, and what it draws when it writes nothing.

mod common;

use std::sync::Arc;

use iceblink::Renderer as _;
use iceblink::{FramePaint, RenderTarget, VectorBackend};
use iceblink_vello::{Choice, VelloOptions, VelloRaster, chosen, for_device};
use iceblink_wgpu::vector::VectorRaster;
use iceblink_wgpu::{Builder, wgpu};

use common::raster::vello;
use iceblink_testing::vector::draw::draw;
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, rect};

const SIDE: i32 = 128;

#[test]
fn rasterisers_made_from_one_renderer_share_it() {
    let Some(gpu) = open_device() else {
        return;
    };

    let one = vello(&gpu);
    let two = VelloRaster::sharing(Arc::clone(one.shared()), &gpu.queue);

    assert!(Arc::ptr_eq(one.shared(), two.shared()));
    assert_eq!(one.memory().fixed, two.memory().fixed);
    assert_eq!(
        (one.memory().vector, two.memory().vector),
        (0, 0),
        "the scratch is allocated by a frame, not by construction"
    );
}

#[test]
fn rasterisers_on_devices_of_two_instances_draw_on_their_own() {
    let Some(first) = open_device() else {
        return;
    };
    let Some(second) = first.beside() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(32.0, 32.0, 64.0, 64.0), paint);
    scene.pass(&[path]);
    let mut one = vello(&first);
    let mut two = vello(&second);

    let drawn = [
        draw(&first, &mut one, &scene),
        draw(&second, &mut two, &scene),
    ];

    assert!(!Arc::ptr_eq(one.shared(), two.shared()));
    for drawn in drawn {
        assert_eq!(drawn.canvas.rgba(64, 64), [255; 4]);
    }
}

#[test]
fn the_renderer_goes_with_the_last_rasteriser_of_its_device() {
    let Some(gpu) = open_device() else {
        return;
    };
    let one = vello(&gpu);
    let shared = Arc::downgrade(one.shared());

    drop(one);

    assert!(shared.upgrade().is_none());
}

#[test]
fn compiled_pipelines_are_kept_in_the_directory_named() {
    let Some(gpu) = open_device() else {
        return;
    };
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("pipelines");
    let _ = std::fs::remove_dir_all(&directory);
    let options = VelloOptions {
        adapter: Some(gpu.adapter.get_info()),
        cache_directory: Some(directory.clone()),
    };

    let raster = VelloRaster::new(&gpu.device, &gpu.queue, &options).expect("vello builds");

    let entries = std::fs::read_dir(&directory).map_or(0, Iterator::count);
    let caches = gpu
        .device
        .features()
        .contains(wgpu::Features::PIPELINE_CACHE);
    assert_eq!(
        entries,
        usize::from(caches),
        "one cache file per adapter that caches"
    );
    drop(raster);
}

#[test]
fn the_fixed_footprint_is_measured() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(8.0, 8.0, 32.0, 32.0), paint);
    scene.pass(&[path]);
    let mut raster = vello(&gpu);

    draw(&gpu, &mut raster, &scene);

    let memory = raster.memory();
    println!(
        "vello fixed: {:.1} MiB, scratch: {:.1} MiB",
        memory.fixed as f64 / 1048576.0,
        memory.vector as f64 / 1048576.0
    );
    assert!(
        gpu.device.generate_allocator_report().is_some(),
        "the device reports its memory"
    );
    assert!(memory.fixed > 0);
}

#[test]
fn a_configured_renderer_holds_no_path_buffers_before_its_first_path_frame() {
    let side = SIDE as u32;
    let mut renderer = Builder::new()
        .offscreen([side, side], wgpu::TextureFormat::Bgra8Unorm)
        .unwrap_or_else(|failure| panic!("no usable graphics device: {failure}"));
    renderer.set_vector_factory(
        Box::new(|gpu, size| {
            for_device(
                gpu.adapter(),
                gpu.device(),
                gpu.queue(),
                size,
                &VelloOptions::default(),
            )
        }),
        VectorBackend::Compute,
        "vello",
    );

    renderer.configure(RenderTarget {
        size: [side, side],
        scale: 1.0,
        opaque: true,
    });

    assert!(
        renderer.vector_status().initialized,
        "the rasteriser is built"
    );
    let fixed = renderer.memory().fixed;
    assert!(
        fixed < 64 * 1024 * 1024,
        "no frame has drawn a path, and {fixed} bytes are held"
    );
}

#[test]
fn a_reused_scratch_does_not_replay_the_previous_frame() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut raster = vello(&gpu);
    let mut first = Scene::new(SIDE);
    let red = first.solid([255, 0, 0, 255]);
    let green = first.solid([0, 255, 0, 255]);
    let square = first.fill(rect(16.0, 16.0, 48.0, 48.0), red);
    let other = first.fill(rect(72.0, 72.0, 40.0, 40.0), green);
    first.pass(&[square, other]);
    let drawn = draw(&gpu, &mut raster, &first);
    assert_eq!(drawn.canvas.rgba(40, 40), [255, 0, 0, 255]);

    // The same frame with the square's paint gone: its ink is still composited.
    let mut second = Scene::new(SIDE);
    let nothing = second.paint(FramePaint::None);
    let green = second.solid([0, 255, 0, 255]);
    let square = second.fill(rect(16.0, 16.0, 48.0, 48.0), nothing);
    let other = second.fill(rect(72.0, 72.0, 40.0, 40.0), green);
    second.pass(&[square, other]);
    let replayed = draw(&gpu, &mut raster, &second);

    assert_eq!(
        replayed.canvas.rgba(40, 40),
        [0, 0, 0, 255],
        "the cleared layer reads empty"
    );
    assert_eq!(replayed.canvas.rgba(92, 92), [0, 255, 0, 255]);
    assert_eq!(raster.last_frame().unpaintable, 1);
}

#[test]
fn the_rasteriser_a_device_gets_draws() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(32.0, 32.0, 64.0, 64.0), paint);
    scene.pass(&[path]);

    let options = VelloOptions {
        adapter: None,
        cache_directory: Some(std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("chosen")),
    };
    let mut raster = for_device(
        &gpu.adapter,
        &gpu.device,
        &gpu.queue,
        [SIDE as u32; 2],
        &options,
    );
    let drawn = draw(&gpu, raster.as_mut(), &scene);

    let expected = match chosen(&gpu.adapter) {
        Choice::Compute => iceblink::VectorBackend::Compute,
        Choice::Coverage => iceblink::VectorBackend::Raster,
    };
    assert_eq!(raster.backend(), expected);
    assert_eq!(drawn.canvas.rgba(64, 64), [255; 4]);
}
