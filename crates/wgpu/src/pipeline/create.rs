//! Building the renderer's shader modules and render pipelines.

use crate::gpu::device::Gpu;
use crate::pipeline::kind::PipelineKind;
use crate::pipeline::layout::Layouts;
use crate::shader::Module;

/// Creates the shader module of `source` on `gpu`.
pub(super) fn shader_module(gpu: &Gpu, source: Module) -> wgpu::ShaderModule {
    gpu.device()
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(source.label()),
            // Parsed at build time, so a launch hands the driver IR.
            source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(source.representation())),
        })
}

/// Builds the pipeline of `kind` for targets of `format` from `module`, through `cache`.
pub(super) fn build(
    gpu: &Gpu,
    layouts: &Layouts,
    cache: Option<&wgpu::PipelineCache>,
    kind: PipelineKind,
    format: wgpu::TextureFormat,
    module: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
    let bind_group_layouts = bind_group_layouts(layouts, kind);
    let pipeline_layout = gpu
        .device()
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(kind.label()),
            bind_group_layouts: &bind_group_layouts,
            immediate_size: 0,
        });
    gpu.device()
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(kind.label()),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(kind.vertex_entry()),
                compilation_options: Default::default(),
                buffers: if kind.takes_order() {
                    ORDER_LAYOUT
                } else {
                    &[]
                },
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(kind.fragment_entry()),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: kind.blend(),
                    write_mask: kind.write_mask(),
                })],
            }),
            multiview_mask: None,
            cache,
        })
}

/// Returns the bind group layouts of `kind`, in group order.
fn bind_group_layouts(
    layouts: &Layouts,
    kind: PipelineKind,
) -> Vec<Option<&wgpu::BindGroupLayout>> {
    let mut bind_group_layouts = Vec::new();
    if kind.uses_tables() {
        bind_group_layouts.push(Some(&layouts.frame));
    }
    if kind.is_instanced() {
        bind_group_layouts.push(Some(&layouts.instances));
        if kind.samples_atlas() {
            bind_group_layouts.push(Some(&layouts.sampled));
        }
    } else if kind.draws_marks() {
        bind_group_layouts.push(Some(&layouts.instances));
        bind_group_layouts.push(Some(if kind == PipelineKind::MarksComposite {
            &layouts.mark_composite
        } else {
            &layouts.marks
        }));
        // A placement reads its cells from an atlas page, bound after its payload.
        if kind.samples_atlas() {
            bind_group_layouts.push(Some(&layouts.sampled));
        }
    } else if kind.composites_vector() {
        bind_group_layouts.push(Some(&layouts.vector));
    } else if kind.samples_through_block() {
        bind_group_layouts.push(Some(&layouts.filtered));
    } else if matches!(kind, PipelineKind::Blit | PipelineKind::BlitUndoSrgb) {
        bind_group_layouts.push(Some(&layouts.loaded));
    }
    bind_group_layouts
}

/// The vertex buffer layout of a draw order, with one entry per instance: an arena slot and the
/// offset the instance is drawn moved by.
pub(crate) const ORDER_LAYOUT: &[Option<wgpu::VertexBufferLayout<'static>>] =
    &[Some(wgpu::VertexBufferLayout {
        array_stride: 12,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 4,
                shader_location: 1,
            },
        ],
    })];
