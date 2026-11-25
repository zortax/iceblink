//! The bind groups a frame's draws bind, made again only when a buffer they read was replaced.

use iceblink::ParamsBlock;

use crate::bind::globals::Globals;
use crate::buffer::chunks::CELLS;
use crate::gpu::device::Gpu;
use crate::pipeline::composite::CompositeParams;
use crate::pipeline::layout::Layouts;
use crate::pipeline::marks::{MarkDraw, MarkKind};
use crate::renderer::buffers::FrameBuffers;

/// A bind group, with the generations of the buffers it was made over.
pub(super) type Keyed<const N: usize> = Option<([u64; N], wgpu::BindGroup)>;

impl FrameBuffers {
    /// Returns the bind group of a filtered draw that samples `view` through `sampler`.
    pub(crate) fn filtered_bind_group(
        &self,
        gpu: &Gpu,
        layouts: &Layouts,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> Option<wgpu::BindGroup> {
        let generation = self.blocks.generation();
        let mut binds = self.filtered_binds.borrow_mut();
        if let Some(bind) = binds.find(generation, view, Some(sampler)) {
            return Some(bind);
        }
        let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iceblink.bind.filtered"),
            layout: &layouts.filtered,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.blocks.binding::<CompositeParams>()?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        binds.keep(generation, view, Some(sampler), &bind);
        Some(bind)
    }

    /// Returns the bind group of the effect parameter blocks.
    pub(crate) fn effect_bind_group(
        &self,
        gpu: &Gpu,
        layouts: &Layouts,
    ) -> Option<wgpu::BindGroup> {
        let generation = self.effect_params.generation();
        if let Some((held, bind)) = self.effect_bind.borrow().as_ref()
            && *held == generation
        {
            return Some(bind.clone());
        }
        let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iceblink.bind.effect"),
            layout: &layouts.effect,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self
                    .effect_params
                    .binding_of(size_of::<ParamsBlock>() as u64)?,
            }],
        });
        *self.effect_bind.borrow_mut() = Some((generation, bind.clone()));
        Some(bind)
    }

    /// Returns the bind group of a vector composite that reads the raster in `scratch`.
    pub(crate) fn vector_bind_group(
        &self,
        gpu: &Gpu,
        layouts: &Layouts,
        scratch: &wgpu::TextureView,
    ) -> Option<wgpu::BindGroup> {
        let generation = self.vectors.generation();
        let mut binds = self.vector_binds.borrow_mut();
        if let Some(bind) = binds.find(generation, scratch, None) {
            return Some(bind);
        }
        let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iceblink.bind.vector"),
            layout: &layouts.vector,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.vectors.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(scratch),
                },
            ],
        });
        binds.keep(generation, scratch, None, &bind);
        Some(bind)
    }

    /// Returns the bind group of the marks of `kind`.
    pub(crate) fn mark_bind_group(
        &self,
        gpu: &Gpu,
        layouts: &Layouts,
        kind: MarkKind,
    ) -> Option<wgpu::BindGroup> {
        let lane = kind.lane();
        let signature = [
            self.mark_draws.generation(),
            self.chunks.payload_generation(lane),
            self.chunks.payload_generation(CELLS),
        ];
        if let Some((held, bind)) = self.mark_binds.borrow().get(lane)?
            && *held == signature
        {
            return Some(bind.clone());
        }
        let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iceblink.bind.marks"),
            layout: &layouts.marks,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.mark_draws.binding::<MarkDraw>()?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.chunks.payload_binding(lane),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.chunks.payload_binding(CELLS),
                },
            ],
        });
        *self.mark_binds.borrow_mut().get_mut(lane)? = Some((signature, bind.clone()));
        Some(bind)
    }

    /// Returns the bind group of the mark composite that reads `bins`.
    pub(crate) fn mark_composite_bind_group(
        &self,
        gpu: &Gpu,
        layouts: &Layouts,
        bins: &wgpu::TextureView,
        bins_generation: u64,
    ) -> Option<wgpu::BindGroup> {
        let signature = [self.mark_draws.generation(), bins_generation];
        if let Some((held, bind)) = self.mark_composite_bind.borrow().as_ref()
            && *held == signature
        {
            return Some(bind.clone());
        }
        let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iceblink.bind.mark_composite"),
            layout: &layouts.mark_composite,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.mark_draws.binding::<MarkDraw>()?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(bins),
                },
            ],
        });
        *self.mark_composite_bind.borrow_mut() = Some((signature, bind.clone()));
        Some(bind)
    }

    /// Returns the bind group of the frame's globals, tables and clip masks.
    pub(crate) fn frame_bind_group(&self, gpu: &Gpu, layouts: &Layouts) -> Option<wgpu::BindGroup> {
        let signature = [
            self.globals.generation(),
            self.clips.generation(),
            self.paints.generation(),
            self.stops.generation(),
            self.spatial.generation(),
            self.links.generation(),
            self.masks.generation(),
        ];
        if let Some((held, bind)) = self.frame_bind.borrow().as_ref()
            && *held == signature
        {
            return Some(bind.clone());
        }
        let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iceblink.bind.frame"),
            layout: &layouts.frame,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.globals.binding::<Globals>()?,
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.clips.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.paints.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.stops.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.spatial.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.links.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(self.masks.alpha_view()),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(self.masks.color_view()),
                },
            ],
        });
        *self.frame_bind.borrow_mut() = Some((signature, bind.clone()));
        Some(bind)
    }

    /// Returns the bind group of the instances of `lane` and the mark draw order.
    pub(crate) fn instance_bind_group(
        &self,
        gpu: &Gpu,
        layouts: &Layouts,
        lane: usize,
    ) -> Option<wgpu::BindGroup> {
        let signature = [self.chunks.generation(lane), self.mark_order.generation()];
        if let Some((held, bind)) = self.instance_binds.borrow().get(lane)?
            && *held == signature
        {
            return Some(bind.clone());
        }
        let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(lane_label(lane)),
            layout: &layouts.instances,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.chunks.binding(lane),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.mark_order.binding(),
                },
            ],
        });
        *self.instance_binds.borrow_mut().get_mut(lane)? = Some((signature, bind.clone()));
        Some(bind)
    }
}

/// Returns the label of the instance bind group of `lane`.
fn lane_label(lane: usize) -> &'static str {
    match lane {
        0 => "iceblink.bind.instances.quads",
        1 => "iceblink.bind.instances.shadows",
        2 => "iceblink.bind.instances.lines",
        3 => "iceblink.bind.instances.mono",
        4 => "iceblink.bind.instances.subpixel",
        5 => "iceblink.bind.instances.color",
        6 => "iceblink.bind.instances.shader_quads",
        7 => "iceblink.bind.instances.marks",
        _ => "iceblink.bind.instances",
    }
}
