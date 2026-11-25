//! Composites of vector-pass targets.

use crate::frame::pass::Recorder;
use crate::frame::pass::bound::{Bound, BoundPass};
use crate::frame::pass::draw::Draw;
use crate::pipeline::kind::PipelineKind;
use crate::vector::VectorTarget;

impl Recorder<'_> {
    /// Composites the vector instances `instances` from the rasteriser's target `target`, and
    /// reports whether they were drawn.
    pub(super) fn vector(
        &mut self,
        pass: &mut BoundPass<'_>,
        context: Draw<'_>,
        target: VectorTarget,
        instances: core::ops::Range<u32>,
    ) -> bool {
        let Some(view) = self.vectors.and_then(|source| source.view(target)) else {
            return false;
        };
        let Some(bind) = self
            .buffers
            .vector_bind_group(self.gpu, self.pipelines.layouts(), view)
        else {
            return false;
        };
        let Some(tables) = context.tables else {
            return false;
        };
        let kind = PipelineKind::VectorComposite;
        let Some(pipeline) = self.pipelines.get(self.gpu, kind, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Kind(kind), pipeline);
        pass.set_bind_group(0, tables, &[context.planned.globals]);
        pass.set_bind_group(1, &bind, &[]);
        pass.draw(0..4, instances);
        true
    }
}
