//! The fill and resolve pipelines.

use crate::vector_coverage::scratch::FORMAT;

/// The WGSL of the fill.
pub(crate) const COVERAGE: &str = include_str!("shader/coverage.wgsl");

/// The WGSL of the conversion from premultiplied to straight colour.
const RESOLVE: &str = include_str!("shader/resolve.wgsl");

/// The pipelines and bind group layouts, built once.
#[derive(Debug)]
pub(crate) struct Pipelines {
    /// Fills one outline into the accumulation texture.
    pub(crate) coverage: wgpu::RenderPipeline,
    /// The layout of the four tables the fill reads.
    pub(crate) coverage_layout: wgpu::BindGroupLayout,
    /// Converts one accumulated layer into straight colour.
    pub(crate) resolve: wgpu::RenderPipeline,
    /// The layout of the accumulated layer the conversion reads.
    pub(crate) resolve_layout: wgpu::BindGroupLayout,
}

impl Pipelines {
    /// Builds both pipelines on `device`.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let coverage_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iceblink.vector.coverage"),
            entries: &[table(0), table(1), table(2), table(3)],
        });
        let resolve_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iceblink.vector.coverage.resolve"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let coverage = build(
            device,
            &coverage_layout,
            COVERAGE,
            "iceblink.vector.coverage",
            ("vs_coverage", "fs_coverage"),
            // Outlines of one pass blend over each other, which is fixed-function only in
            // premultiplied form.
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        let resolve = build(
            device,
            &resolve_layout,
            RESOLVE,
            "iceblink.vector.coverage.resolve",
            ("vs_resolve", "fs_resolve"),
            None,
        );
        Self {
            coverage,
            coverage_layout,
            resolve,
            resolve_layout,
        }
    }
}

/// Returns the layout entry of a table read one texel at a time by both stages.
fn table(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Uint,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Builds one pipeline from `source` with the vertex and fragment `entries`.
fn build(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &str,
    label: &'static str,
    entries: (&str, &str),
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(entries.0),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(entries.1),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: FORMAT,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_shader_divides_by_the_width_the_tables_are_allocated_at() {
        let declared = format!(
            "const TABLE_TEXELS_WIDE: u32 = {}u;",
            crate::upload::TEXELS_WIDE
        );
        assert!(super::COVERAGE.contains(&declared));
    }
}
