//! The check of a presented native image against the kept target.

use iceblink::geom::{Device, Size};

use crate::gpu::device::Gpu;
use crate::renderer::readback;

/// Compares `image`, an acquired native image at `slot`, with `composed` over `size` texels, and
/// logs the first texel that differs.
// Runs after the copy is submitted and before the image is released, since nothing may read an
// image the display server holds.
pub(crate) fn check(
    gpu: &Gpu,
    image: &wgpu::Texture,
    slot: u32,
    composed: &wgpu::Texture,
    size: Size<i32, Device>,
) {
    let shown = readback::read(gpu, image, image.format(), size);
    let kept = readback::read(gpu, composed, composed.format(), size);
    'rows: for y in 0..size.height {
        for x in 0..size.width {
            let (left, right) = (shown.rgba(x, y), kept.rgba(x, y));
            if left != right {
                tracing::error!(
                    x,
                    y,
                    shown = ?left,
                    composed = ?right,
                    slot,
                    "a presented image differs from the kept target"
                );
                break 'rows;
            }
        }
    }

    // The readback left wgpu tracking the image as a copy source, and the release barrier expects
    // a colour attachment, so an empty pass over the image tracks it as one again.
    let view = image.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = gpu
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("iceblink.present.verified"),
        });
    drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("iceblink.present.verified"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
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
    }));
    gpu.queue().submit([encoder.finish()]);
}
