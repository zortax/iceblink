//! The wgpu renderer drawing the same frames, for comparison.

use std::sync::MutexGuard;

use iceblink::Frame;
use iceblink_testing::golden::{RgbaImage, Tolerance, difference};
use iceblink_testing::gpu::{device_lock, skip};
use iceblink_wgpu::{Builder, WgpuRenderer, wgpu};

use crate::common::{SIDE, configured, draw};

/// A wgpu renderer of a `SIDE` square offscreen target, holding the device lock.
pub(crate) struct Gpu {
    pub(crate) renderer: WgpuRenderer,
    _lock: MutexGuard<'static, ()>,
}

/// Returns a wgpu renderer, or `None` when no device opens.
pub(crate) fn gpu() -> Option<Gpu> {
    let lock = device_lock();
    match Builder::new().offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm) {
        Ok(renderer) => Some(Gpu {
            renderer: configured(renderer),
            _lock: lock,
        }),
        Err(failure) => skip(&format!("no usable graphics device: {failure}")),
    }
}

impl Gpu {
    /// Draws `frame` and returns the image presented.
    pub(crate) fn drawn(&mut self, frame: &Frame) -> RgbaImage {
        draw(&mut self.renderer, frame);
        let pixels = self
            .renderer
            .read_presented()
            .expect("an offscreen image can be read back");
        RgbaImage::from_texels(SIDE, SIDE, pixels.bytes(), pixels.is_bgra())
    }
}

/// Asserts that `cpu` draws what `gpu` draws within `tolerance`.
#[track_caller]
pub(crate) fn assert_close(what: &str, gpu: &RgbaImage, cpu: &RgbaImage, tolerance: Tolerance) {
    let found = difference(gpu, cpu, tolerance).expect("the images have one size");
    if found.pixels > tolerance.max_pixels {
        let directory = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("against_gpu");
        let _ = std::fs::create_dir_all(&directory);
        let _ =
            iceblink_testing::golden::write_png(&directory.join(format!("{what}.gpu.png")), gpu);
        let _ =
            iceblink_testing::golden::write_png(&directory.join(format!("{what}.cpu.png")), cpu);
    }
    assert!(
        found.pixels <= tolerance.max_pixels,
        "{what}: the CPU differs from the GPU by {found:?}, {tolerance:?} allowed"
    );
}
