//! The end-to-end conformance suite on the reference compositor and the reference renderer.

use iceblink::{Compositor, Renderer};
use iceblink::{RenderCapabilities, RenderTarget};
use iceblink_core::CompositorImpl;
use iceblink_testing::capture::CaptureRenderer;
use iceblink_testing::conformance::draw::DrawTarget;
use iceblink_testing::conformance::end_to_end::{
    Pair, adopted_motion_equals_whole, driven_motion_equals_whole, frames_reach_a_renderer,
    incremental_equals_whole, pairs_agree,
};
use iceblink_testing::golden::{RgbaImage, Tolerance};
use iceblink_testing::gpu::{device_lock, skip};
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

fn core() -> Box<dyn Compositor> {
    Box::new(CompositorImpl::new())
}

#[test]
fn incremental_frames_equal_whole_redraws() {
    let _device = device_lock();
    incremental_equals_whole(&core, &offscreen);
}

#[test]
fn compositor_scrolling_during_client_changes_equals_whole_redraws() {
    let _device = device_lock();
    let shifted = driven_motion_equals_whole(&core, &offscreen);
    assert!(
        shifted > 20,
        "the compositor's motion moves the port by shifts, in {shifted} frames"
    );
}

#[test]
fn adopting_offsets_while_input_waits_equals_whole_redraws() {
    let _device = device_lock();
    let shifted = adopted_motion_equals_whole(&core, &offscreen);
    assert!(
        shifted > 10,
        "the compositor's motion moves the port by shifts, in {shifted} frames"
    );
}

#[test]
fn frames_planned_for_every_fast_path_and_for_none_agree() {
    let _device = device_lock();
    let full = Pair {
        compositor: &core,
        target: &offscreen,
        caps: None,
    };
    let minimal = Pair {
        caps: Some(RenderCapabilities {
            max_texture_size: 4096,
            ..RenderCapabilities::MINIMAL
        }),
        ..full
    };
    pairs_agree(
        full,
        minimal,
        Tolerance {
            max_channel: 2,
            max_pixels: 0,
        },
    );
}

#[test]
fn random_frames_reach_the_capture_renderer() {
    frames_reach_a_renderer(&core, &|size| {
        let mut renderer = CaptureRenderer::new();
        renderer.configure(RenderTarget {
            size,
            scale: 1.0,
            opaque: true,
        });
        Box::new(renderer)
    });
}
