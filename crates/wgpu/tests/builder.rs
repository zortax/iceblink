//! Builder options: a device the application opened, the pipeline cache directory and latency
//! marks.

mod common;

use std::sync::{Arc, Mutex};

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{SIDE, device_lock};
use common::present::target;
use iceblink::FrameOutcome;
use iceblink::Renderer as _;
use iceblink_wgpu::{Builder, wgpu};

fn frame() -> iceblink::Frame {
    let mut builder = FrameBuilder::new();
    let red = builder.solid(opaque(255, 0, 0));
    builder.quad(filled(rect(0.0, 0.0, 32.0, 32.0), red));
    builder.finish()
}

#[test]
fn a_renderer_draws_on_a_device_the_application_opened() {
    let _guard = device_lock();
    let first = Builder::new()
        .offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm)
        .expect("a device");
    let gpu = Arc::clone(first.gpu());
    let mut renderer = Builder::from_gpu(Arc::clone(&gpu))
        .offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm)
        .expect("the same device");

    assert!(Arc::ptr_eq(renderer.gpu(), &gpu));

    renderer.configure(target(SIDE));
    assert!(matches!(
        renderer.draw(&frame()),
        FrameOutcome::Presented(_)
    ));
    assert_eq!(
        renderer.read_presented().unwrap().rgba(8, 8),
        [255, 0, 0, 255]
    );
}

#[test]
fn the_pipeline_cache_is_kept_in_the_directory_given() {
    let _guard = device_lock();
    let directory = std::env::temp_dir().join(format!("iceblink-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let mut renderer = Builder::new()
        .with_pipeline_cache_dir(&directory)
        .offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm)
        .expect("a device");

    // A launch without a cache builds most pipelines on a thread, and a frame writes the cache.
    renderer.configure(target(SIDE));
    renderer.draw(&frame());

    let supports = renderer
        .gpu()
        .device()
        .features()
        .contains(wgpu::Features::PIPELINE_CACHE);
    let written = std::fs::read_dir(&directory).map_or(0, |entries| entries.count());
    let _ = std::fs::remove_dir_all(&directory);

    assert_eq!(
        written > 0,
        supports,
        "a cache file exists exactly where the device keeps caches"
    );
}

#[test]
fn latency_marks_reach_the_application() {
    let _guard = device_lock();
    let marks: Arc<Mutex<Vec<&'static str>>> = Arc::default();
    let seen = Arc::clone(&marks);
    let mut renderer = Builder::new()
        .with_latency_marks(Arc::new(move |name, _| seen.lock().unwrap().push(name)))
        .offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm)
        .expect("a device");

    renderer.configure(target(SIDE));
    renderer.draw(&frame());

    let marks = marks.lock().unwrap();
    assert_eq!(marks.first(), Some(&"draw.in"));
    assert!(marks.contains(&"pres.out"));
}
