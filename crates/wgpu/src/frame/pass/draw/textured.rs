//! Draws that read one target through a filtering sampler: blurs and composites.

use crate::frame::pass::Recorder;
use crate::frame::pass::bound::{Bound, BoundPass};
use crate::frame::pass::draw::Draw;
use crate::frame::target::TargetRef;
use crate::pipeline::kind::PipelineKind;

impl Recorder<'_> {
    /// Draws `source` through the pipeline `kind` with the parameter offset `params`, and reports
    /// whether it was drawn.
    ///
    /// With `tables`, the frame tables are bound first, at the pass's globals.
    pub(super) fn textured(
        &mut self,
        pass: &mut BoundPass<'_>,
        context: Draw<'_>,
        kind: PipelineKind,
        source: TargetRef,
        params: u32,
        tables: bool,
    ) -> bool {
        let Some(view) = self.view(source) else {
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
        let Some(pipeline) = self.pipelines.get(self.gpu, kind, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Kind(kind), pipeline);
        let group = match context.tables.filter(|_| tables) {
            Some(bind_group) => {
                pass.set_bind_group(0, bind_group, &[context.planned.globals]);
                1
            }
            None => 0,
        };
        pass.set_bind_group(group, &bind, &[params]);
        pass.draw(0..4, 0..1);
        true
    }
}
