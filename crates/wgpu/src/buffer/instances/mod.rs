//! Instance and draw-order storage: a texture table read by index, and a vertex buffer read as
//! instance attributes.

mod order;

use bytemuck::Pod;

use crate::gpu::device::Gpu;
use crate::upload::{ROW, TEXEL, UploadBelt, allocate_rows, row_limit, write_run};

pub(crate) use self::order::OrderBuffer;

/// A table of instance records in an `rgba32uint` texture, read by index.
#[derive(Debug)]
pub(crate) struct InstanceTable {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    label: &'static str,
    rows: u32,
    generation: u64,
    overflowed: bool,
}

impl InstanceTable {
    const MINIMUM_ROWS: u32 = 1;

    pub(crate) fn new(gpu: &Gpu, label: &'static str) -> Self {
        let (texture, view) = allocate_rows(gpu.device(), label, Self::MINIMUM_ROWS);
        Self {
            texture,
            view,
            label,
            rows: Self::MINIMUM_ROWS,
            generation: 1,
            overflowed: false,
        }
    }

    /// Records the upload of `values`, each padded to whole texels, and returns the bytes
    /// uploaded.
    ///
    /// The texture grows to the next power of two of rows that holds the records, up to the
    /// device's limit.
    pub(crate) fn upload<T: Pod>(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        values: &[T],
    ) -> u64 {
        let element = size_of::<T>();
        let stride = element.div_ceil(TEXEL) * TEXEL;
        let bytes = values.len() * stride;
        // A table past the row limit of the device keeps the records that fit. The write below is
        // cut to the texture, and a record past it reads zeros and draws nothing.
        let limit = row_limit(gpu.device());
        let wanted = bytes.div_ceil(ROW).max(Self::MINIMUM_ROWS as usize);
        if wanted > limit as usize && !std::mem::replace(&mut self.overflowed, true) {
            tracing::error!(
                table = self.label,
                rows = wanted,
                limit,
                "a frame holds more records than a texture can; the records past the limit are \
                 not drawn"
            );
        }
        let rows = wanted.min(limit as usize) as u32;
        if rows > self.rows {
            self.rows = rows.next_power_of_two().min(limit);
            let (texture, view) = allocate_rows(gpu.device(), self.label, self.rows);
            self.texture = texture;
            self.view = view;
            self.generation = self.generation.wrapping_add(1);
        }
        if values.is_empty() {
            return 0;
        }
        let packed: &[u8] = bytemuck::cast_slice(values);
        if stride == element {
            let written = write_run(gpu.device(), belt, encoder, &self.texture, 0, packed);
            return gpu.uploaded(written);
        }
        let mut run = vec![0_u8; bytes];
        for (index, record) in packed.chunks_exact(element).enumerate() {
            run[index * stride..index * stride + element].copy_from_slice(record);
        }
        gpu.uploaded(write_run(
            gpu.device(),
            belt,
            encoder,
            &self.texture,
            0,
            &run,
        ))
    }

    /// Returns a number that changes whenever the table gets a new texture.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::TextureView(&self.view)
    }

    pub(crate) fn capacity(&self) -> u64 {
        u64::from(self.rows) * ROW as u64
    }

    /// Shrinks the texture back to the fewest rows, and returns the bytes freed.
    pub(crate) fn shrink(&mut self, gpu: &Gpu) -> u64 {
        if self.rows <= Self::MINIMUM_ROWS {
            return 0;
        }
        let freed = (u64::from(self.rows) - u64::from(Self::MINIMUM_ROWS)) * ROW as u64;
        let (texture, view) = allocate_rows(gpu.device(), self.label, Self::MINIMUM_ROWS);
        self.texture = texture;
        self.view = view;
        self.rows = Self::MINIMUM_ROWS;
        self.generation = self.generation.wrapping_add(1);
        freed
    }
}
