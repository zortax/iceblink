//! Instanced batches of the frame's lanes.

use iceblink::{Batch, BatchKind};

use crate::buffer::chunks::OrderEntry;
use crate::frame::pass::Recorder;
use crate::frame::pass::bound::{Bound, BoundPass};
use crate::frame::pass::draw::Draw;
use crate::pipeline::kind::PipelineKind;
use crate::renderer::buffers::FrameBuffers;

impl<'frame> Recorder<'frame> {
    /// Draws an instanced batch, and reports whether it was drawn.
    pub(super) fn batch<'pass>(
        &mut self,
        pass: &mut BoundPass<'pass>,
        context: Draw<'_>,
        batch: &Batch,
    ) -> bool
    where
        'frame: 'pass,
    {
        let (kind, page) = match batch.kind {
            BatchKind::Quads => (PipelineKind::Quad, None),
            BatchKind::Shadows => (PipelineKind::Shadow, None),
            BatchKind::Lines => (PipelineKind::Line, None),
            BatchKind::Mono { page } => (PipelineKind::MonoSprite, Some(page)),
            BatchKind::Subpixel { page } => (PipelineKind::SubpixelSprite, Some(page)),
            BatchKind::Color { page } => (PipelineKind::ColorSprite, Some(page)),
            BatchKind::ShaderQuads { shader, params } => {
                return self.shaded(pass, context, [shader, params], batch.range.clone());
            }
            // Every other kind is planned, not drawn as a batch.
            _ => return false,
        };
        let range = batch.range.clone();
        if range.is_empty() {
            return false;
        }
        let Some(tables) = context.tables else {
            return false;
        };
        let Some(lane) = FrameBuffers::lane(kind) else {
            return false;
        };
        let Some(instances) =
            self.buffers
                .instance_bind_group(self.gpu, self.pipelines.layouts(), lane)
        else {
            return false;
        };
        let atlas = match page {
            None => None,
            // A sprite whose page is gone draws nothing rather than another page's texels.
            Some(page) => match self.pages.bind_group(page) {
                Some(bind_group) => Some(bind_group),
                None => return false,
            },
        };
        let Some(pipeline) = self.pipelines.get(self.gpu, kind, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Kind(kind), pipeline);
        pass.set_bind_group(0, tables, &[context.planned.globals]);
        pass.set_bind_group(1, &instances, &[]);
        if let Some(bind_group) = atlas {
            pass.set_bind_group(2, bind_group, &[]);
        }
        let buffers = self.buffers;
        let order = &buffers.orders[lane];
        let stride = core::mem::size_of::<OrderEntry>() as u64;
        pass.set_vertex_buffer(0, order.slice_from(u64::from(range.start) * stride));
        pass.draw(0..4, 0..(range.end - range.start));
        true
    }
}
