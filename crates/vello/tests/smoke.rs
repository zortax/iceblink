//! A known pattern drawn through vello and read back.

mod common;

use iceblink_wgpu::vector::VectorRaster;

use common::raster::vello;
use iceblink_testing::vector::draw::draw;
use iceblink_testing::vector::open_device;
use iceblink_testing::vector::scene::{Scene, rect};

const SIDE: i32 = 128;

/// The four quadrants of the pattern, and the colour each comes back as.
const PATTERN: [((f64, f64), [u8; 4]); 4] = [
    ((0.0, 0.0), [255, 0, 0, 255]),
    ((64.0, 0.0), [0, 255, 0, 255]),
    ((0.0, 64.0), [0, 0, 255, 255]),
    ((64.0, 64.0), [255, 255, 0, 255]),
];

#[test]
fn the_pattern_comes_back_through_vello() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paths: Vec<u32> = PATTERN
        .iter()
        .map(|&((x, y), colour)| {
            let paint = scene.solid(colour);
            scene.fill(rect(x, y, 64.0, 64.0), paint)
        })
        .collect();
    scene.pass(&paths);
    let drawn = draw(&gpu, &mut vello(&gpu), &scene);

    drawn.outcome.expect("the frame rasterises");
    for ((x, y), colour) in PATTERN {
        let at = (x as i32 + 32, y as i32 + 32);
        assert_eq!(
            drawn.canvas.rgba(at.0, at.1),
            colour,
            "the quadrant at {at:?}"
        );
    }
}

#[test]
fn the_fixed_footprint_and_the_scratch_are_two_numbers() {
    let Some(gpu) = open_device() else {
        return;
    };
    let mut scene = Scene::new(SIDE);
    let paint = scene.solid([255, 255, 255, 255]);
    let path = scene.fill(rect(16.0, 16.0, 64.0, 64.0), paint);
    scene.pass(&[path]);
    let mut raster = vello(&gpu);

    draw(&gpu, &mut raster, &scene);

    let memory = raster.memory();
    assert!(memory.vector > 0, "the scratch is reported");
    // The fixed footprint comes from the device's allocator report, which wgpu publishes on
    // Vulkan and DX12.
    assert!(
        gpu.device.generate_allocator_report().is_some(),
        "the device reports its memory"
    );
    assert!(
        memory.fixed > 64 * 1024 * 1024,
        "vello's fixed buffers take hundreds of megabytes, and {} was reported",
        memory.fixed
    );
    assert!(memory.vector < memory.fixed);
}
