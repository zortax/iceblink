//! A uniform buffer of fixed-stride blocks staged per frame and bound at dynamic offsets.

use bytemuck::Pod;

use crate::gpu::device::Gpu;
use crate::upload::UploadBelt;

/// A uniform buffer of fixed-stride blocks, staged per frame and bound at dynamic offsets.
#[derive(Debug)]
pub(crate) struct SlotBuffer {
    buffer: Option<wgpu::Buffer>,
    label: &'static str,
    stride: u64,
    staged: Vec<u8>,
    taken: u32,
    generation: u64,
}

impl SlotBuffer {
    pub(crate) fn new<T: Pod>(gpu: &Gpu, label: &'static str) -> Self {
        Self::with_stride(gpu, label, size_of::<T>() as u64)
    }

    /// Returns an empty buffer of blocks of `size` bytes.
    ///
    /// The stride is `size` rounded up to the device's uniform offset alignment.
    pub(crate) fn with_stride(gpu: &Gpu, label: &'static str, size: u64) -> Self {
        let alignment = u64::from(gpu.device().limits().min_uniform_buffer_offset_alignment);
        Self {
            buffer: None,
            label,
            stride: size.next_multiple_of(alignment.max(1)),
            staged: Vec::new(),
            taken: 0,
            generation: 0,
        }
    }

    /// Drops the staged blocks.
    pub(crate) fn reset(&mut self) {
        self.staged.clear();
        self.taken = 0;
    }

    pub(crate) fn stage<T: Pod>(&mut self, value: &T) -> u32 {
        self.stage_bytes(bytemuck::bytes_of(value))
    }

    /// Stages `bytes` in the next block, and returns the block's offset.
    pub(crate) fn stage_bytes(&mut self, bytes: &[u8]) -> u32 {
        let offset = u64::from(self.taken) * self.stride;
        self.staged.resize(offset as usize, 0);
        self.staged.extend_from_slice(bytes);
        self.staged.resize((offset + self.stride) as usize, 0);
        self.taken += 1;
        offset as u32
    }

    /// Records the upload of the staged blocks, and returns the bytes uploaded.
    ///
    /// The buffer is replaced by one of the next power of two of bytes when it is too small.
    pub(crate) fn upload_with(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
    ) -> u64 {
        if self.staged.is_empty() {
            return 0;
        }
        let needed = self.staged.len() as u64;
        let outgrown = match &self.buffer {
            Some(buffer) => buffer.size() < needed,
            None => true,
        };
        if outgrown {
            self.buffer = Some(gpu.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: needed.next_power_of_two(),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.generation = self.generation.wrapping_add(1);
        }
        let buffer = self
            .buffer
            .as_ref()
            .expect("a non-empty upload has allocated a buffer");
        gpu.uploaded(belt.write(gpu.device(), encoder, buffer, 0, &self.staged))
    }

    /// Returns the binding of one block of `T`, or `None` before the first upload.
    pub(crate) fn binding<T: Pod>(&self) -> Option<wgpu::BindingResource<'_>> {
        let buffer = self.buffer.as_ref()?;
        Some(wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: wgpu::BufferSize::new(size_of::<T>() as u64),
        }))
    }

    /// Returns the binding of one block of `size` bytes, or `None` before the first upload.
    pub(crate) fn binding_of(&self, size: u64) -> Option<wgpu::BindingResource<'_>> {
        let buffer = self.buffer.as_ref()?;
        Some(wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: wgpu::BufferSize::new(size),
        }))
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.buffer.as_ref().map_or(0, wgpu::Buffer::size)
    }

    /// Returns a number that changes whenever the buffer is replaced or dropped.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Drops the buffer and the staged blocks, and returns the bytes freed.
    pub(crate) fn release(&mut self) -> u64 {
        let freed = self.bytes();
        if self.buffer.take().is_some() {
            self.generation = self.generation.wrapping_add(1);
        }
        self.staged.clear();
        self.staged.shrink_to_fit();
        freed
    }
}
