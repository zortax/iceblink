//! A vertex buffer of draw orders, read as instance attributes.

use bytemuck::Pod;

use crate::gpu::device::Gpu;
use crate::upload::UploadBelt;

/// A vertex buffer of draw-order entries, read as instance attributes.
#[derive(Debug)]
pub(crate) struct OrderBuffer {
    buffer: wgpu::Buffer,
    label: &'static str,
    capacity: u64,
}

impl OrderBuffer {
    const MINIMUM: u64 = 256;

    pub(crate) fn new(gpu: &Gpu, label: &'static str) -> Self {
        Self {
            buffer: allocate_order(gpu, label, Self::MINIMUM),
            label,
            capacity: Self::MINIMUM,
        }
    }

    /// Records the upload of `values`, and returns the bytes uploaded.
    ///
    /// The buffer grows to the next power of two of bytes that holds them.
    pub(crate) fn upload<T: Pod>(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        values: &[T],
    ) -> u64 {
        let all: &[u8] = bytemuck::cast_slice(values);
        if all.is_empty() {
            return 0;
        }
        let needed = all.len() as u64;
        if needed > self.capacity {
            self.capacity = needed.next_power_of_two().max(Self::MINIMUM);
            self.buffer = allocate_order(gpu, self.label, self.capacity);
        }
        gpu.uploaded(belt.write(gpu.device(), encoder, &self.buffer, 0, all))
    }

    /// Returns the buffer from byte `offset` on, with the offset clamped to the capacity.
    pub(crate) fn slice_from(&self, offset: u64) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(offset.min(self.capacity)..)
    }

    pub(crate) fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Shrinks the buffer back to the smallest size, and returns the bytes freed.
    pub(crate) fn shrink(&mut self, gpu: &Gpu) -> u64 {
        if self.capacity <= Self::MINIMUM {
            return 0;
        }
        let freed = self.capacity - Self::MINIMUM;
        self.buffer = allocate_order(gpu, self.label, Self::MINIMUM);
        self.capacity = Self::MINIMUM;
        freed
    }
}

fn allocate_order(gpu: &Gpu, label: &'static str, size: u64) -> wgpu::Buffer {
    gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
