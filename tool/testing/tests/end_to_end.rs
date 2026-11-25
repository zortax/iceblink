//! The end-to-end conformance suite on the naive compositor and iceblink-wgpu.

use iceblink::RenderTarget;
use iceblink::{Compositor, Renderer};
use iceblink_testing::conformance::draw::DrawTarget;
use iceblink_testing::conformance::end_to_end::incremental_equals_whole;
use iceblink_testing::golden::RgbaImage;
use iceblink_testing::gpu::{device_lock, skip};
use iceblink_testing::naive::NaiveCompositor;
use iceblink_wgpu::vector_coverage::CoverageRaster;
use iceblink_wgpu::{Builder, WgpuRenderer, wgpu};

/// An offscreen renderer with the portable path rasteriser.
struct Offscreen(WgpuRenderer);

impl DrawTarget for Offscreen {
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

/// Returns an offscreen target of `size`, or `None` when no device opens.
///
/// The device lock must be held.
fn offscreen(size: [u32; 2]) -> Option<Box<dyn DrawTarget>> {
    let mut renderer = match Builder::new().offscreen(size, wgpu::TextureFormat::Rgba8Unorm) {
        Ok(renderer) => renderer,
        Err(failure) => return skip(&format!("no usable graphics device: {failure}")),
    };
    let gpu = renderer.gpu().clone();
    let paths = CoverageRaster::new(gpu.device(), gpu.queue(), size[0], size[1]);
    renderer.set_vector_raster(Box::new(paths), "coverage");
    renderer.configure(RenderTarget {
        size,
        scale: 1.0,
        opaque: true,
    });
    Some(Box::new(Offscreen(renderer)))
}

fn naive() -> Box<dyn Compositor> {
    Box::new(NaiveCompositor::new())
}

#[test]
fn incremental_frames_equal_whole_redraws() {
    let _device = device_lock();
    incremental_equals_whole(&naive, &offscreen);
}
