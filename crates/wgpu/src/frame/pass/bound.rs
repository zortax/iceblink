//! A render pass that skips pipeline and bind group changes that change nothing.

use crate::effect::EffectKey;
use crate::pipeline::kind::PipelineKind;

/// The pipeline a pass has bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bound {
    Kind(PipelineKind),
    Effect(EffectKey),
}

/// How many bind groups a draw binds at most.
const BIND_SLOTS: usize = 4;

/// A render pass, with the pipeline and bind groups it has bound.
pub(super) struct BoundPass<'pass> {
    pass: wgpu::RenderPass<'pass>,
    pipeline: Option<Bound>,
    groups: [Option<(wgpu::BindGroup, Option<u32>)>; BIND_SLOTS],
}

impl<'pass> BoundPass<'pass> {
    pub(super) fn new(pass: wgpu::RenderPass<'pass>) -> Self {
        Self {
            pass,
            pipeline: None,
            groups: Default::default(),
        }
    }

    /// Binds `pipeline`, known as `key`, unless it is bound already.
    pub(super) fn set_pipeline(&mut self, key: Bound, pipeline: &wgpu::RenderPipeline) {
        if self.pipeline != Some(key) {
            self.pass.set_pipeline(pipeline);
            self.pipeline = Some(key);
        }
    }

    pub(super) fn set_vertex_buffer(&mut self, slot: u32, slice: wgpu::BufferSlice<'pass>) {
        self.pass.set_vertex_buffer(slot, slice);
    }

    pub(super) fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.pass.set_scissor_rect(x, y, width, height);
    }

    /// Binds `group` with `offsets` at `index`, unless they are bound already.
    pub(super) fn set_bind_group(&mut self, index: u32, group: &wgpu::BindGroup, offsets: &[u32]) {
        debug_assert!(
            offsets.len() <= 1,
            "every bind group here has one dynamic offset at most"
        );
        let offset = offsets.first().copied();
        let held = &mut self.groups[index as usize];
        if held
            .as_ref()
            .is_some_and(|(bound, at)| bound == group && *at == offset)
        {
            return;
        }
        self.pass.set_bind_group(index, group, offsets);
        *held = Some((group.clone(), offset));
    }

    pub(super) fn draw(
        &mut self,
        vertices: core::ops::Range<u32>,
        instances: core::ops::Range<u32>,
    ) {
        self.pass.draw(vertices, instances);
    }
}
