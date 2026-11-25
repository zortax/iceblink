//! Reading a scratch layer back to the host.

use crate::gpu::TestDevice;

/// Reads back the straight texels of the top-left `width` by `height` of the layer `view` shows.
pub fn layer(gpu: &TestDevice, view: &wgpu::TextureView, width: u32, height: u32) -> Vec<[u8; 4]> {
    let device = &gpu.device;
    let (width, height) = (width.max(1), height.max(1));
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let row = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    blit(device, &mut encoder, view, &target);
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        size,
    );
    gpu.queue.submit([encoder.finish()]);

    texels(device, &buffer, row, size)
}

/// Records a draw that copies the texels `source` shows one to one into `target`.
fn blit(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::TextureView,
    target: &wgpu::Texture,
) {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("iceblink.vector.test.readback"),
        source: wgpu::ShaderSource::Wgsl(BLIT.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
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
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            }),
        ),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(source),
        }],
    });

    let target_view = target.create_view(&Default::default());
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &target_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(&pipeline);
    pass.set_bind_group(0, &bind, &[]);
    pass.draw(0..3, 0..1);
}

/// Maps `buffer` once the device is done with it, and returns its texels, `row` bytes apart.
fn texels(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    row: u32,
    size: wgpu::Extent3d,
) -> Vec<[u8; 4]> {
    buffer.slice(..).map_async(wgpu::MapMode::Read, |result| {
        result.expect("the readback maps");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the device finishes the readback");
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .expect("the readback is mapped");

    let mut texels = Vec::with_capacity((size.width * size.height) as usize);
    for y in 0..size.height {
        let start = (y * row) as usize;
        for x in 0..size.width as usize {
            let at = start + x * 4;
            texels.push([mapped[at], mapped[at + 1], mapped[at + 2], mapped[at + 3]]);
        }
    }
    texels
}

/// A full-screen triangle that copies texels one to one.
const BLIT: &str = r"
@group(0) @binding(0) var source: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    return vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(position.xy), 0);
}
";
