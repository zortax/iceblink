//! Filter shaders over group targets.

use crate::effect::EffectKey;
use crate::frame::pass::Recorder;
use crate::frame::pass::bound::{Bound, BoundPass};
use crate::frame::pass::draw::Draw;
use crate::frame::target::TargetRef;

impl Recorder<'_> {
    /// Draws the filter shader `shader` over `source` with the dynamic offsets of its parameters
    /// and its own block, and reports whether it was drawn.
    pub(super) fn effect_filter(
        &mut self,
        pass: &mut BoundPass<'_>,
        context: Draw<'_>,
        source: TargetRef,
        shader: EffectKey,
        [params, block]: [u32; 2],
    ) -> bool {
        let Some(view) = self.view(source) else {
            return false;
        };
        let Some(read) = self.buffers.filtered_bind_group(
            self.gpu,
            self.pipelines.layouts(),
            view,
            self.sampler,
        ) else {
            return false;
        };
        let Some(own) = self
            .buffers
            .effect_bind_group(self.gpu, self.pipelines.layouts())
        else {
            return false;
        };
        let Some(pipeline) = self.pipelines.effect(self.gpu, shader, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Effect(shader), pipeline);
        pass.set_bind_group(0, &read, &[params]);
        pass.set_bind_group(1, &own, &[block]);
        pass.draw(0..4, 0..1);
        true
    }
}
