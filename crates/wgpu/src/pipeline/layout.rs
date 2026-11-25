//! The bind group layouts every pipeline is built from.

use crate::gpu::device::Gpu;

/// The bind group layouts of the renderer's pipelines.
#[derive(Clone, Debug)]
pub(crate) struct Layouts {
    pub(crate) frame: wgpu::BindGroupLayout,
    pub(crate) instances: wgpu::BindGroupLayout,
    pub(crate) sampled: wgpu::BindGroupLayout,
    pub(crate) loaded: wgpu::BindGroupLayout,
    pub(crate) filtered: wgpu::BindGroupLayout,
    pub(crate) vector: wgpu::BindGroupLayout,
    pub(crate) effect: wgpu::BindGroupLayout,
    pub(crate) marks: wgpu::BindGroupLayout,
    pub(crate) mark_composite: wgpu::BindGroupLayout,
}

impl Layouts {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        let device = gpu.device();
        Self {
            frame: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.frame"),
                entries: &[
                    dynamic_uniform(0),
                    table(1),
                    table(2),
                    table(3),
                    table(4),
                    table(5),
                    mask(6),
                    mask(7),
                ],
            }),
            effect: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.effect"),
                entries: &[dynamic_uniform(0)],
            }),
            marks: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.marks"),
                entries: &[dynamic_uniform(0), table(1), table(2)],
            }),
            mark_composite: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.mark_composite"),
                entries: &[
                    dynamic_uniform(0),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            }),
            instances: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.instances"),
                entries: &[table(0), table(1)],
            }),
            sampled: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.sampled"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            }),
            filtered: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.filtered"),
                entries: &[
                    dynamic_uniform(0),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            }),
            vector: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.vector"),
                entries: &[
                    table(0),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            }),
            loaded: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("iceblink.bind.loaded"),
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
            }),
        }
    }
}

/// Returns the entry of a uniform block that both stages read at a dynamic offset.
fn dynamic_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Returns the entry of an unfilterable float texture that the fragment stage reads.
fn mask(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Returns the entry of a table that both stages read one texel at a time.
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
