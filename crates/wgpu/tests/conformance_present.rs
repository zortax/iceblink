//! The present conformance suite on the reference renderer's offscreen presenter.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, MutexGuard};

use iceblink::PresentMode;
use iceblink::Renderer;
use iceblink_testing::conformance::present::PresentRig;
use iceblink_testing::golden::RgbaImage;
use iceblink_testing::gpu::{device_lock, skip};
use iceblink_wgpu::{Acquisition, Builder, Offscreen, WgpuRenderer, wgpu};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// An offscreen renderer whose presents wait while the gate is closed.
struct Rig {
    renderer: WgpuRenderer,
    open: Arc<AtomicBool>,
    _guard: MutexGuard<'static, ()>,
}

impl PresentRig for Rig {
    fn renderer(&mut self) -> &mut dyn Renderer {
        &mut self.renderer
    }

    fn presented(&mut self) -> Option<RgbaImage> {
        let pixels = self.renderer.read_presented()?;
        let size = pixels.size();
        Some(RgbaImage::from_texels(
            size.width as u32,
            size.height as u32,
            pixels.bytes(),
            pixels.is_bgra(),
        ))
    }

    fn hold_presents(&mut self, held: bool) -> bool {
        self.open.store(!held, Ordering::Relaxed);
        true
    }

    fn time_out_acquisitions(&mut self, times: u32) -> bool {
        let Some(offscreen) = self
            .renderer
            .wgpu_presenter()
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<Offscreen>())
        else {
            return false;
        };
        offscreen.inject_fault(Acquisition::Timeout, times);
        true
    }

    fn lose_device(&mut self) -> bool {
        self.renderer
            .gpu()
            .loss()
            .report(wgpu::DeviceLostReason::Unknown, "lost by the test");
        true
    }
}

/// Returns a rig of `mode` and `size` on a device of its own.
fn rig(mode: PresentMode, size: [u32; 2]) -> Option<Box<dyn PresentRig>> {
    rig_from(Builder::new, mode, size)
}

/// Returns a rig whose renderer draws on a device it was handed, which it does not replace after a
/// loss.
fn adopted_rig(mode: PresentMode, size: [u32; 2]) -> Option<Box<dyn PresentRig>> {
    let guard = device_lock();
    let opened = match Builder::new().offscreen(size, FORMAT) {
        Ok(renderer) => renderer,
        Err(failure) => return skip(&format!("no usable graphics device: {failure}")),
    };
    let gpu = Arc::clone(opened.gpu());
    drop(opened);
    drop(guard);
    rig_from(move || Builder::from_gpu(Arc::clone(&gpu)), mode, size)
}

/// Returns a rig of `mode` and `size` on a renderer `builder` starts, or `None` after reporting
/// the skip.
fn rig_from(
    builder: impl Fn() -> Builder,
    mode: PresentMode,
    size: [u32; 2],
) -> Option<Box<dyn PresentRig>> {
    let guard = device_lock();
    let built = match mode {
        PresentMode::KeptTarget => builder().offscreen(size, FORMAT),
        PresentMode::BufferAge => builder().offscreen_buffer_age(size, FORMAT, 3),
    };
    let mut renderer = match built {
        Ok(renderer) => renderer,
        Err(failure) => return skip(&format!("no usable graphics device: {failure}")),
    };
    let open = Arc::new(AtomicBool::new(true));
    let gate = Arc::clone(&open);
    renderer.set_present_gate(Box::new(move || gate.load(Ordering::Relaxed)));
    Some(Box::new(Rig {
        renderer,
        open,
        _guard: guard,
    }))
}

iceblink_testing::present_suite!(rig);

/// The device-loss case on a renderer that reports its device unavailable.
mod adopted {
    iceblink_testing::present_suite!(@cases super::adopted_rig; device_loss);
}
