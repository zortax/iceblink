//! Copying a texture back to memory as RGBA.

use crate::golden::RgbaImage;

/// Copies the whole of `texture` into memory, as RGBA bytes in the order red, green, blue, alpha.
///
/// The bytes are the stored values, so an sRGB-encoded texture reads back encoded.
///
/// # Panics
///
/// Panics if the texture's format is not one of the 8-bit RGBA or BGRA formats, or if the copy
/// cannot be mapped.
pub fn read_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> RgbaImage {
    let bgra = match texture.format() {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
        other => panic!("cannot read back a texture of format {other:?}"),
    };
    let (width, height) = (texture.width(), texture.height());
    let row = width * 4;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("iceblink.test-readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("iceblink.test-readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the readback copy completes");
    let view = slice
        .get_mapped_range()
        .expect("the readback buffer was mapped by the wait above");

    let mut pixels = Vec::with_capacity((row * height) as usize);
    for line in view.chunks_exact(padded as usize) {
        pixels.extend_from_slice(&line[..row as usize]);
    }
    drop(view);
    buffer.unmap();

    RgbaImage::from_texels(width, height, &pixels, bgra)
}
