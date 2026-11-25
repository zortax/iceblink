//! Batches of shader quads drawn with an application shader.

use crate::buffer::chunks::{OrderEntry, SHADER_QUADS_LANE};
use crate::frame::pass::Recorder;
use crate::frame::pass::bound::{Bound, BoundPass};
use crate::frame::pass::draw::Draw;

impl<'frame> Recorder<'frame> {
    /// Draws the shader quads `range` with the shader and parameter slots `[shader, params]`, and
    /// reports whether they were drawn.
    pub(super) fn shaded<'pass>(
        &mut self,
        pass: &mut BoundPass<'pass>,
        context: Draw<'_>,
        [shader, params]: [u32; 2],
        range: core::ops::Range<u32>,
    ) -> bool
    where
        'frame: 'pass,
    {
        if range.is_empty() {
            return false;
        }
        // A slot that holds no shader draws nothing.
        let Some(key) = (self.effects)(shader) else {
            tracing::debug!(
                shader,
                "a shader quad refers to a shader slot that holds no shader"
            );
            return false;
        };
        let Some(tables) = context.tables else {
            return false;
        };
        let Some(offset) = self.buffers.effect_offset(params) else {
            self.pipelines
                .effects()
                .note_undrawable(key, "this frame staged no block for its parameter slot");
            return false;
        };
        let Some(block) = self
            .buffers
            .effect_bind_group(self.gpu, self.pipelines.layouts())
        else {
            return false;
        };
        let lane = SHADER_QUADS_LANE;
        let Some(instances) =
            self.buffers
                .instance_bind_group(self.gpu, self.pipelines.layouts(), lane)
        else {
            return false;
        };
        let Some(pipeline) = self.pipelines.effect(self.gpu, key, context.format) else {
            return false;
        };
        pass.set_pipeline(Bound::Effect(key), pipeline);
        pass.set_bind_group(0, tables, &[context.planned.globals]);
        pass.set_bind_group(1, &instances, &[]);
        pass.set_bind_group(2, &block, &[offset]);
        let buffers = self.buffers;
        let stride = core::mem::size_of::<OrderEntry>() as u64;
        pass.set_vertex_buffer(
            0,
            buffers.orders[lane].slice_from(u64::from(range.start) * stride),
        );
        pass.draw(0..4, 0..(range.end - range.start));
        true
    }
}
