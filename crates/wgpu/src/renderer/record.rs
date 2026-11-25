//! Recording and submitting a frame's encoder work.

use iceblink::Frame;
use iceblink::geom::{Point, Rect, Size};

use crate::frame::build::FramePlan;
use crate::frame::pass::{Recorded, Recorder};
use crate::frame::segment::Segment;
use crate::frame::target::Composed;
use crate::present::WgpuPresenter;
use crate::renderer::WgpuRenderer;
use crate::target::scene_texture::SceneTexture;

impl WgpuRenderer {
    /// Returns a new encoder for a frame's work.
    pub(crate) fn frame_encoder(&self) -> wgpu::CommandEncoder {
        self.gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("iceblink.frame"),
            })
    }

    /// Submits `encoder`, and ties the frame's retired ranges and staging to the submission.
    pub(crate) fn submit_frame(&mut self, mut encoder: wgpu::CommandEncoder) {
        if let Some(timer) = &mut self.timer {
            timer.finish(&mut encoder);
        }
        self.buffers.uploader.finish();
        self.gpu.queue().submit([encoder.finish()]);
        if let Some(timer) = &mut self.timer {
            timer.submitted();
        }
        self.buffers.chunks.submitted(&self.gpu);
        self.buffers.uploader.recall();
        self.latency.mark("sub.out");
    }

    /// Records the draws of `segments` of `plan` into the composed target, the acquired `image`
    /// in buffer-age mode, and returns what was recorded.
    pub(crate) fn record(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        plan: &FramePlan,
        segments: &[Segment],
        image: Option<u32>,
        frame: &Frame,
    ) -> Recorded {
        // Borrows single fields, so the timer can still be borrowed mutably below.
        let Some(composed) = composed_of(&self.kept, self.presenter.as_ref(), image) else {
            return Recorded::default();
        };
        let pipelines = &mut *self.pipelines.borrow_mut();
        let externals = &self.externals;
        let external = |index: u32| {
            frame
                .externals
                .get(index as usize)
                .and_then(|held| externals.view(held))
        };
        let effects = &self.effects;
        let effect = |shader: u32| effects.get(shader);
        let mark_extent = Size::new(
            frame.mark_plan.extent[0] as i32,
            frame.mark_plan.extent[1] as i32,
        );

        Recorder {
            gpu: &self.gpu,
            pipelines,
            buffers: &self.buffers,
            pages: &self.pages,
            pool: &self.groups,
            composed,
            sampler: &self.sampler,
            externals: &external,
            effects: &effect,
            vectors: self.vectors.as_deref(),
            marks: &self.marks_scratch,
            mark_extent,
            timer: self.timer.as_mut(),
        }
        .record(encoder, plan, segments)
    }

    /// Returns the target frames are composed into: the kept target, or the acquired `image` in
    /// buffer-age mode.
    pub(crate) fn composed(&self, image: Option<u32>) -> Option<Composed<'_>> {
        composed_of(&self.kept, self.presenter.as_ref(), image)
    }
}

/// Returns the target frames are composed into, from the renderer's fields.
fn composed_of<'a>(
    kept: &'a SceneTexture,
    presenter: &'a dyn WgpuPresenter,
    image: Option<u32>,
) -> Option<Composed<'a>> {
    match image {
        None => Some(Composed {
            texture: kept.texture(),
            view: kept.view(),
            format: kept.format(),
            used: kept.used(),
        }),
        Some(image) => {
            let texture = presenter.image_texture(image)?;
            let [width, height] = presenter.extent();
            Some(Composed {
                texture,
                view: presenter.image_view(image)?,
                format: presenter.formats().present_attachment(),
                used: Rect::new(Point::new(0, 0), Size::new(width as i32, height as i32)),
            })
        }
    }
}
