//! Copies of texture rows into a staging buffer, read back once the device is idle.

use std::ops::Range;

use iceblink::geom::{Device, Size};

use crate::gpu::device::Gpu;
use crate::renderer::readback::Pixels;

/// Reads every texel of `texture`, which holds `size` texels of `format`, waiting for the device.
pub(crate) fn read(
    gpu: &Gpu,
    texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
    size: Size<i32, Device>,
) -> Pixels {
    let mut into = Pixels::nothing();
    let mut staging = Staging::new();
    let whole = crate::frame::damage::every_row(size.height.max(1) as u32);
    read_bands(gpu, texture, format, size, &whole, &mut staging, &mut into);
    into
}

/// A readback buffer, reused while it is large enough.
#[derive(Debug, Default)]
pub(crate) struct Staging {
    buffer: Option<wgpu::Buffer>,
}

impl Staging {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Returns a buffer of at least `bytes`, allocating a larger one when needed.
    fn of(&mut self, gpu: &Gpu, bytes: u64) -> &wgpu::Buffer {
        let held = self
            .buffer
            .as_ref()
            .is_some_and(|held| held.size() >= bytes);
        if !held {
            self.buffer = Some(gpu.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("iceblink.readback"),
                size: bytes,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }
        self.buffer.as_ref().expect("just allocated")
    }
}

/// Reads the rows `bands` of `texture`, which holds `size` texels of `format`, into `into`,
/// waiting for the device.
///
/// `into` is resized to `size`, and its other rows are kept when its extent and channel order
/// are unchanged. Rows past the texture are left out.
pub(crate) fn read_bands(
    gpu: &Gpu,
    texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
    size: Size<i32, Device>,
    bands: &[Range<u32>],
    staging: &mut Staging,
    into: &mut Pixels,
) {
    let width = size.width.max(1) as u32;
    let height = size.height.max(1) as u32;
    let bgra = matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );
    into.resize(Size::new(width as i32, height as i32), bgra);

    // Cut to the texture and dropped where nothing is left, so that a band naming rows past the
    // bottom is a band that reads what there is rather than a copy the device refuses.
    let bands: Vec<Range<u32>> = bands
        .iter()
        .map(|band| band.start.min(height)..band.end.min(height))
        .filter(|band| band.start < band.end)
        .collect();
    if bands.is_empty() {
        return;
    }

    let padded = padded_bytes_per_row(width);
    let mut offsets = Vec::with_capacity(bands.len());
    let mut used = 0_u64;
    for band in &bands {
        used = used.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT);
        offsets.push(used);
        used += u64::from(padded) * u64::from(band.end - band.start);
    }
    let buffer = staging.of(gpu, used.max(1));

    let mut encoder = gpu
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("iceblink.readback"),
        });
    for (band, offset) in bands.iter().zip(&offsets) {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: band.start,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: *offset,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(band.end - band.start),
                },
            },
            wgpu::Extent3d {
                width,
                height: band.end - band.start,
                depth_or_array_layers: 1,
            },
        );
    }
    gpu.queue().submit([encoder.finish()]);

    let slice = buffer.slice(..used);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.wait();

    let view = slice
        .get_mapped_range()
        .expect("the readback buffer was mapped by the wait above");
    let row = (width * 4) as usize;
    for (band, offset) in bands.iter().zip(&offsets) {
        for (index, line) in (band.start..band.end).enumerate() {
            let from = *offset as usize + index * padded as usize;
            let to = line as usize * row;
            into.bytes[to..to + row].copy_from_slice(&view[from..from + row]);
        }
    }
    drop(view);
    buffer.unmap();
}

/// Returns the bytes of a row of `width` texels, padded to the copy alignment.
fn padded_bytes_per_row(width: u32) -> u32 {
    let unpadded = width * 4;
    unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}
