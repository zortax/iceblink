//! Slices drawn by the wgpu renderer on a device, and the images they present.

use iceblink_testing::golden::RgbaImage;
use iceblink_testing::gpu::skip;
use iceblink_wgpu::vector_coverage::CoverageRaster;
use iceblink_wgpu::{Builder, WgpuRenderer, wgpu};

use crate::common::slice::{SIDE, Slice};

/// Returns a slice drawing into an offscreen renderer, or `None` when no device opens.
pub(crate) fn slice() -> Option<Slice<WgpuRenderer>> {
    slice_keeping(true)
}

/// Returns a slice drawing into an offscreen renderer that keeps vector rasters if `kept`, or
/// `None` when no device opens.
pub(crate) fn slice_keeping(kept: bool) -> Option<Slice<WgpuRenderer>> {
    match Builder::new().offscreen([SIDE, SIDE], wgpu::TextureFormat::Rgba8Unorm) {
        Ok(mut renderer) => {
            renderer.set_rasters_kept(kept);
            eprintln!("adapter: {}", renderer.gpu().describe());
            let gpu = renderer.gpu().clone();
            let paths = CoverageRaster::new(gpu.device(), gpu.queue(), SIDE, SIDE);
            renderer.set_vector_raster(Box::new(paths), "coverage");
            Some(Slice::new(renderer))
        }
        Err(failure) => skip(&format!("no usable graphics device: {failure}")),
    }
}

/// Returns the image the renderer presented last.
pub(crate) fn presented(slice: &Slice<WgpuRenderer>) -> RgbaImage {
    let pixels = slice
        .renderer
        .read_presented()
        .expect("an offscreen image can be read back");
    RgbaImage::from_texels(SIDE, SIDE, pixels.bytes(), pixels.is_bgra())
}
