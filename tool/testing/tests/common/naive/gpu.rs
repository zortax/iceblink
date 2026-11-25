//! A GPU renderer that draws a compositor's frames offscreen and reads them back.

use std::sync::MutexGuard;

use iceblink::{Built, Frame, FrameClock, FrameRequest, OutputId, RenderTarget, VectorBackend};
use iceblink::{Compositor, Renderer};
use iceblink_testing::FrameValidator;
use iceblink_testing::gpu::{device_lock, skip};
use iceblink_testing::validate::describe;
use iceblink_wgpu::vector_coverage::CoverageRaster;
use iceblink_wgpu::{Builder, Pixels, WgpuRenderer, wgpu};

/// An offscreen renderer with a path rasteriser, and the validator its frames pass through.
pub(crate) struct Rig {
    pub(crate) renderer: WgpuRenderer,
    validator: FrameValidator,
    frame: Frame,
    now: u64,
    _guard: Option<MutexGuard<'static, ()>>,
}

impl Rig {
    /// Returns a rig on the test device, holding the device lock, or `None` when the test skips.
    pub(crate) fn open(size: [u32; 2]) -> Option<Self> {
        Self::opened(size, Some(device_lock()))
    }

    /// Returns a second rig while another rig holds the device lock.
    pub(crate) fn beside(size: [u32; 2]) -> Option<Self> {
        Self::opened(size, None)
    }

    fn opened(size: [u32; 2], guard: Option<MutexGuard<'static, ()>>) -> Option<Self> {
        let mut renderer = match Builder::new().offscreen(size, wgpu::TextureFormat::Bgra8Unorm) {
            Ok(renderer) => renderer,
            Err(failure) => return skip(&format!("no usable graphics device: {failure}")),
        };
        eprintln!("adapter: {}", renderer.gpu().describe());
        renderer.set_vector_factory(
            Box::new(|gpu, size| {
                Box::new(CoverageRaster::new(
                    gpu.device(),
                    gpu.queue(),
                    size[0],
                    size[1],
                ))
            }),
            VectorBackend::Raster,
            "coverage",
        );
        renderer.configure(RenderTarget {
            size,
            scale: 1.0,
            opaque: true,
        });

        Some(Self {
            renderer,
            validator: FrameValidator::new(),
            frame: Frame::default(),
            now: 0,
            _guard: guard,
        })
    }

    /// Builds the next frame of `output`, checks it, draws it and reports its outcome.
    ///
    /// Returns the presented image, or `None` when the output was idle.
    pub(crate) fn draw(
        &mut self,
        compositor: &mut dyn Compositor,
        output: OutputId,
    ) -> Option<Pixels> {
        self.now += 16_000_000;
        let caps = self.renderer.capabilities();
        self.validator.set_capabilities(Some(caps));
        let request = FrameRequest {
            caps: &caps,
            age: self.renderer.target_age(),
            clock: FrameClock {
                now_ns: self.now,
                present_ns: self.now + 8_000_000,
                interval_ns: 16_000_000,
            },
            subpixel: None,
        };

        if compositor.build_frame(output, &request, &mut self.frame) != Built::Frame {
            return None;
        }

        if let Err(violations) = self.validator.check(&self.frame) {
            panic!(
                "frame {} breaks the frame contract:\n{}",
                self.frame.id.0,
                describe(&violations)
            );
        }

        let outcome = self.renderer.draw(&self.frame);
        compositor.frame_done(output, self.frame.id, &outcome);
        assert!(
            outcome.stats().is_some(),
            "the frame was presented: {outcome:?}"
        );
        self.renderer.read_presented()
    }
}
