//! The draw conformance suite on the reference renderer, on the driver the environment selects.

use std::sync::MutexGuard;

use iceblink::RenderTarget;
use iceblink::Renderer;
use iceblink_testing::conformance::draw::DrawTarget;
use iceblink_testing::golden::RgbaImage;
use iceblink_testing::gpu::{device_lock, skip};
use iceblink_vello::{VelloOptions, VelloRaster};
use iceblink_wgpu::{Builder, WgpuRenderer, wgpu};

/// An offscreen renderer with vello for paths, holding the device lock.
struct Offscreen {
    renderer: WgpuRenderer,
    _guard: MutexGuard<'static, ()>,
}

impl DrawTarget for Offscreen {
    fn renderer(&mut self) -> &mut dyn Renderer {
        &mut self.renderer
    }

    fn presented(&mut self) -> RgbaImage {
        let pixels = self
            .renderer
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

/// Returns an offscreen draw target of `size`, or `None` after reporting the skip.
fn offscreen(size: [u32; 2]) -> Option<Box<dyn DrawTarget>> {
    let guard = device_lock();
    let mut renderer = match Builder::new().offscreen(size, wgpu::TextureFormat::Rgba8Unorm) {
        Ok(renderer) => renderer,
        Err(failure) => return skip(&format!("no usable graphics device: {failure}")),
    };
    let gpu = renderer.gpu().clone();
    let paths = VelloRaster::new(gpu.device(), gpu.queue(), &VelloOptions::default())
        .expect("vello builds on the device");
    renderer.set_vector_raster(Box::new(paths), "vello");
    renderer.configure(RenderTarget {
        size,
        scale: 1.0,
        opaque: true,
    });
    Some(Box::new(Offscreen {
        renderer,
        _guard: guard,
    }))
}

iceblink_testing::draw_suite!(offscreen);
