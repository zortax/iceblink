//! External quads, drawn from client textures.

use crate::frame::pass::Recorder;
use crate::frame::pass::bound::{Bound, BoundPass};
use crate::frame::pass::draw::Draw;
use crate::pipeline::kind::PipelineKind;

impl Recorder<'_> {
    /// Draws external quad `index` with the parameter offset `params`, and reports whether it was
    /// drawn.
    pub(super) fn external(
        &mut self,
        pass: &mut BoundPass<'_>,
        context: Draw<'_>,
        index: u32,
        params: u32,
    ) -> bool {
        let Some(view) = (self.externals)(index) else {
            return false;
        };
        let Some(bind) = self.buffers.filtered_bind_group(
            self.gpu,
            self.pipelines.layouts(),
            view,
            self.sampler,
        ) else {
            return false;
        };
        let Some(tables) = context.tables else {
            return false;
        };
        let kind = PipelineKind::External;
        let Some(pipeline) = self.pipelines.get(self.gpu, kind, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Kind(kind), pipeline);
        pass.set_bind_group(0, tables, &[context.planned.globals]);
        pass.set_bind_group(1, &bind, &[params]);
        pass.draw(0..4, 0..1);
        true
    }
}
