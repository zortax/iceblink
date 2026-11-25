//! The draws of one round.

use crate::vector::{ResourcedPlan, VectorError};

use super::CoverageRaster;

impl CoverageRaster {
    /// Sorts the collected passes of `round` into the layer each one was placed in.
    pub(super) fn group(&mut self, plan: &ResourcedPlan, round: core::ops::Range<usize>) {
        let layers = (self.scratch.layers() as usize).max(1);
        for bucket in &mut self.layered {
            bucket.clear();
        }
        self.layered.resize_with(layers, Vec::new);
        for (span, index) in round.take(self.spans.len()).enumerate() {
            let target = plan.passes[index].target.0;
            if let Some(bucket) = usize::try_from(target)
                .ok()
                .and_then(|layer| self.layered.get_mut(layer))
            {
                bucket.push(span);
            }
        }
    }

    /// Records the fills of every layer of the round and one conversion per layer.
    ///
    /// # Errors
    ///
    /// Returns [`VectorError::Allocation`] when a layer the round uses is not allocated.
    // One conversion per layer: the passes sharing a layer have bins that do not meet, so one draw
    // converts them all.
    pub(super) fn record(&self, encoder: &mut wgpu::CommandEncoder) -> Result<(), VectorError> {
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iceblink.vector.coverage"),
            layout: &self.pipelines.coverage_layout,
            entries: &[
                entry(0, self.buffers.items.binding()),
                entry(1, self.buffers.segments.binding()),
                entry(2, self.buffers.runs.binding()),
                entry(3, self.buffers.bands.binding()),
            ],
        });
        for (layer, indices) in self.layered.iter().enumerate() {
            if indices.is_empty() {
                continue;
            }
            let layer = layer as u32;
            let (Some(accumulation), Some(straight)) = (
                self.scratch.accumulation(layer),
                self.scratch.straight(layer),
            ) else {
                return Err(VectorError::Allocation {
                    detail: format!("no scratch layer {layer} is allocated"),
                });
            };
            {
                let mut render = begin(encoder, accumulation, "iceblink.vector.coverage");
                render.set_pipeline(&self.pipelines.coverage);
                render.set_bind_group(0, &bind, &[]);
                for &index in indices {
                    let (first, count) = self.spans[index];
                    if count > 0 {
                        render.draw(0..4, first..first + count);
                    }
                }
            }
            let resolve = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("iceblink.vector.coverage.resolve"),
                layout: &self.pipelines.resolve_layout,
                entries: &[entry(0, wgpu::BindingResource::TextureView(accumulation))],
            });
            let mut render = begin(encoder, straight, "iceblink.vector.coverage.resolve");
            render.set_pipeline(&self.pipelines.resolve);
            render.set_bind_group(0, &resolve, &[]);
            render.draw(0..4, 0..1);
        }
        Ok(())
    }
}

/// Returns the bind group entry of `resource` at `binding`.
fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

/// Opens a render pass that keeps what the attachment holds.
// The attachment holds this round's clear, and outlines blend over each other.
fn begin<'encoder>(
    encoder: &'encoder mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    label: &'static str,
) -> wgpu::RenderPass<'encoder> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
