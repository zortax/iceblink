//! Tables stored as `rgba32uint` textures, written in whole rows or in runs of texels.

mod rows;
mod run;

use bytemuck::Pod;

use crate::upload::belt::{Refused, TexelWrite, UploadBelt};

pub use self::rows::{ROW, TEXEL, TEXELS_WIDE, allocate_rows, row_limit};
pub use self::run::write_run;

/// A table in an `rgba32uint` texture, which grows to the most it has held.
///
/// The texture always has at least one row, so a bind group can refer to it whatever a frame
/// holds. A table holds at most [`limit`](Self::limit) texels, and a record past them reads
/// zeros.
// A texture read with `textureLoad` gives random access on devices without storage buffers, such
// as a GL 3.3 context or WebGL 2.
#[derive(Debug)]
pub struct TableTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// The label drivers report the texture by.
    label: &'static str,
    rows: u32,
    generation: u64,
    /// Whether a table past the limit was reported.
    overflowed: bool,
}

impl TableTexture {
    /// The fewest rows a table has.
    const MINIMUM_ROWS: u32 = 1;

    /// Returns an empty table labelled `label`.
    pub fn new(device: &wgpu::Device, label: &'static str) -> Self {
        let (texture, view) = allocate_rows(device, label, Self::MINIMUM_ROWS);
        Self {
            texture,
            view,
            label,
            rows: Self::MINIMUM_ROWS,
            generation: 1,
            overflowed: false,
        }
    }

    /// Returns how many texels a table can hold on `device`.
    pub fn limit(device: &wgpu::Device) -> usize {
        row_limit(device) as usize * TEXELS_WIDE as usize
    }

    /// Records a copy of all `values` into the table, and returns the bytes written.
    ///
    /// The values fill whole texels from the first. The table grows to the next power of two of
    /// rows when it is too small.
    ///
    /// # Errors
    ///
    /// Returns [`Refused`] when the device refuses a staging mapping.
    pub fn upload<T: Pod>(
        &mut self,
        device: &wgpu::Device,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        values: &[T],
    ) -> Result<u64, Refused> {
        self.upload_range(device, belt, encoder, values, 0..values.len())
    }

    /// Records a copy of the rows that hold `values[range]` into the table, and returns the
    /// bytes written.
    ///
    /// A table that grows is written whole. The rows are cut to the ones the table holds.
    ///
    /// # Errors
    ///
    /// Returns [`Refused`] when the device refuses a staging mapping.
    pub fn upload_range<T: Pod>(
        &mut self,
        device: &wgpu::Device,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        values: &[T],
        range: core::ops::Range<usize>,
    ) -> Result<u64, Refused> {
        let all = self.fitted(device, bytemuck::cast_slice(values));
        let grew = self.grow(device, all.len().div_ceil(ROW) as u32);
        if all.is_empty() || (range.is_empty() && !grew) {
            return Ok(0);
        }

        let element = size_of::<T>();
        let (start, end) = if grew {
            (0, values.len())
        } else {
            (range.start, range.end)
        };
        let held = all.len().div_ceil(ROW);
        let first = ((start * element) / ROW).min(held - 1);
        let last = ((end * element).div_ceil(ROW)).clamp(first + 1, held);

        // A write covers whole rows, so the tail row is padded with zeros. A write of one row is
        // narrowed to the texels it holds.
        let from = first * ROW;
        let to = (last * ROW).min(all.len());
        let wide = if last - first == 1 {
            ((to - from).div_ceil(TEXEL) as u32).min(TEXELS_WIDE)
        } else {
            TEXELS_WIDE
        };
        let mut rows = vec![0_u8; (last - first) * wide as usize * TEXEL];
        rows[..to - from].copy_from_slice(&all[from..to]);

        let write = TexelWrite {
            target: &self.texture,
            mip: 0,
            origin: (0, first as u32),
            extent: (wide, (last - first) as u32),
            texel: TEXEL as u32,
        };
        belt.write_texels(device, encoder, &write, &rows)
    }

    /// Returns the rows of `all` that a table on `device` holds.
    ///
    /// The first time a table is cut short, an error is logged.
    // A record past the kept rows reads zeros, since `textureLoad` outside a texture is defined.
    fn fitted<'a>(&mut self, device: &wgpu::Device, all: &'a [u8]) -> &'a [u8] {
        let limit = row_limit(device);
        let wanted = all.len().div_ceil(ROW);
        if wanted <= limit as usize {
            return all;
        }

        if !std::mem::replace(&mut self.overflowed, true) {
            tracing::error!(
                table = self.label,
                rows = wanted,
                limit,
                "a table holds more rows than a texture can; the records past the limit are not \
                 drawn"
            );
        }
        &all[..limit as usize * ROW]
    }

    /// Grows the texture to hold `needed` rows, and reports whether it grew.
    ///
    /// The new row count is the next power of two, up to the device's limit.
    fn grow(&mut self, device: &wgpu::Device, needed: u32) -> bool {
        if needed <= self.rows {
            return false;
        }

        let rows = needed
            .next_power_of_two()
            .min(row_limit(device))
            .max(Self::MINIMUM_ROWS);
        self.reallocate(device, rows);
        true
    }

    /// Replaces the texture with an empty one of `rows` rows.
    fn reallocate(&mut self, device: &wgpu::Device, rows: u32) {
        let (texture, view) = allocate_rows(device, self.label, rows);
        self.texture = texture;
        self.view = view;
        self.rows = rows;
        self.generation = self.generation.wrapping_add(1);
    }

    /// Returns the binding of the table's view.
    pub fn binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::TextureView(&self.view)
    }

    /// Returns the bytes the texture holds.
    pub fn capacity(&self) -> u64 {
        u64::from(self.rows) * ROW as u64
    }

    /// Returns a number that changes whenever the table gets a new texture.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Shrinks the texture back to the fewest rows, and returns the bytes freed.
    pub fn shrink(&mut self, device: &wgpu::Device) -> u64 {
        if self.rows <= Self::MINIMUM_ROWS {
            return 0;
        }
        let freed = self.capacity() - u64::from(Self::MINIMUM_ROWS) * ROW as u64;
        self.reallocate(device, Self::MINIMUM_ROWS);
        freed
    }
}
