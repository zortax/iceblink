//! The CPU renderer and iceblink-wgpu as targets of the conformance suites.

use iceblink::RenderTarget;
use iceblink::Renderer;
use iceblink_cpu::CpuRenderer;
use iceblink_testing::conformance::draw::DrawTarget;
use iceblink_testing::golden::RgbaImage;
use iceblink_testing::gpu::skip;
use iceblink_vello::{VelloOptions, VelloRaster};
use iceblink_wgpu::{Builder, WgpuRenderer, wgpu};

use crate::common::presented;

/// A CPU renderer presenting into a pixmap in kept-target mode.
struct OnCpu(CpuRenderer);

impl DrawTarget for OnCpu {
    fn renderer(&mut self) -> &mut dyn Renderer {
        &mut self.0
    }

    fn presented(&mut self) -> RgbaImage {
        presented(&mut self.0)
    }
}

/// Returns a configured CPU renderer of `size`.
pub(crate) fn on_cpu(size: [u32; 2]) -> Option<Box<dyn DrawTarget>> {
    let mut renderer = CpuRenderer::offscreen(size);
    renderer.configure(target(size));
    Some(Box::new(OnCpu(renderer)))
}

/// An offscreen iceblink-wgpu renderer with vello for paths.
struct OnGpu(WgpuRenderer);

impl DrawTarget for OnGpu {
    fn renderer(&mut self) -> &mut dyn Renderer {
        &mut self.0
    }

    fn presented(&mut self) -> RgbaImage {
        let pixels = self
            .0
            .read_presented()
            .expect("an offscreen image can be read back");
        let size = pixels.size();
        RgbaImage::from_texels(
            size.width as u32,
            size.height as u32,
            pixels.bytes(),
            pixels.is_bgra(),
        )
    }
}

/// Returns a configured iceblink-wgpu renderer of `size` on a device of its own, or `None`
/// when no device opens.
///
/// The renderer is used only while the device lock is held.
pub(crate) fn on_gpu(size: [u32; 2]) -> Option<Box<dyn DrawTarget>> {
    let mut renderer = match Builder::new().offscreen(size, wgpu::TextureFormat::Rgba8Unorm) {
        Ok(renderer) => renderer,
        Err(failure) => return skip(&format!("no usable graphics device: {failure}")),
    };
    let gpu = renderer.gpu().clone();
    let raster = VelloRaster::new(gpu.device(), gpu.queue(), &VelloOptions::default())
        .expect("vello builds on the device");
    renderer.set_vector_raster(Box::new(raster), "vello");
    renderer.configure(target(size));
    Some(Box::new(OnGpu(renderer)))
}

fn target(size: [u32; 2]) -> RenderTarget {
    RenderTarget {
        size,
        scale: 1.0,
        opaque: true,
    }
}
