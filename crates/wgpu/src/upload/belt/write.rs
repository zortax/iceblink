//! Writes into buffers and textures through the belt.

use crate::upload::belt::{Refused, UploadBelt};

/// A rectangle of one level of a texture that a write fills.
#[derive(Clone, Copy, Debug)]
pub struct TexelWrite<'a> {
    /// The texture written.
    pub target: &'a wgpu::Texture,
    /// The mip level written.
    pub mip: u32,
    /// The top-left texel of the rectangle.
    pub origin: (u32, u32),
    /// The width and height, in texels.
    pub extent: (u32, u32),
    /// The bytes of one texel.
    pub texel: u32,
}

impl UploadBelt {
    /// Records a copy of `bytes` into `target` at `target_offset`, and returns the bytes written.
    ///
    /// # Panics
    ///
    /// Panics when the size or the offset is not a multiple of `COPY_BUFFER_ALIGNMENT`.
    ///
    /// # Errors
    ///
    /// Returns [`Refused`] when the device refuses a staging mapping.
    pub fn write(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Buffer,
        target_offset: u64,
        bytes: &[u8],
    ) -> Result<u64, Refused> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let size = bytes.len() as u64;
        assert!(
            size.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
            "upload size {size} is not copy-buffer aligned"
        );
        assert!(
            target_offset.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
            "upload offset {target_offset} is not copy-buffer aligned"
        );

        let one_shot = size >= Self::MAXIMUM;
        let mut written = 0;
        for piece in bytes.chunks(Self::MAXIMUM as usize) {
            self.write_piece(
                device,
                encoder,
                target,
                target_offset + written,
                piece,
                one_shot,
            )?;
            written += piece.len() as u64;
        }

        self.uploaded += size;
        Ok(size)
    }

    /// Records a copy of the tightly packed texels `bytes` into the rectangle of `write`, and
    /// returns the bytes written.
    ///
    /// # Panics
    ///
    /// Panics when `bytes` does not hold exactly the rectangle's texels.
    ///
    /// # Errors
    ///
    /// Returns [`Refused`] in the cases [`write`](Self::write) does.
    pub fn write_texels(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        write: &TexelWrite<'_>,
        bytes: &[u8],
    ) -> Result<u64, Refused> {
        let row = u64::from(write.extent.0) * u64::from(write.texel);
        assert_eq!(bytes.len() as u64, row * u64::from(write.extent.1));
        self.write_rows(device, encoder, write, row, bytes)
    }

    /// Records a copy of rows of texels that start `stride` bytes apart in `bytes` into the
    /// rectangle of `write`, and returns the bytes written.
    ///
    /// Nothing is written when `stride` is shorter than a row or `bytes` holds fewer rows than
    /// the rectangle.
    ///
    /// # Errors
    ///
    /// Returns [`Refused`] in the cases [`write`](Self::write) does.
    pub fn write_rows(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        write: &TexelWrite<'_>,
        stride: u64,
        bytes: &[u8],
    ) -> Result<u64, Refused> {
        let (width, height) = write.extent;
        if width == 0 || height == 0 {
            return Ok(0);
        }
        let row = u64::from(width) * u64::from(write.texel);
        if stride < row || (bytes.len() as u64) < stride * u64::from(height - 1) + row {
            return Ok(0);
        }

        let padded = padded_row(row);
        let one_shot = padded * u64::from(height) >= Self::MAXIMUM;
        // A row longer than the largest piece is a piece of its own.
        let band = u32::try_from(Self::MAXIMUM / padded)
            .unwrap_or(u32::MAX)
            .max(1);
        let mut first = 0;
        while first < height {
            let rows = band.min(height - first);
            let piece = TexelWrite {
                origin: (write.origin.0, write.origin.1 + first),
                extent: (width, rows),
                ..*write
            };
            let start = (u64::from(first) * stride) as usize;
            self.write_band(device, encoder, &piece, stride, &bytes[start..], one_shot)?;
            first += rows;
        }

        let written = row * u64::from(height);
        self.uploaded += written;
        Ok(written)
    }

    /// Records a copy of `bytes`, which fit one chunk, into `target` at `target_offset`.
    fn write_piece(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Buffer,
        target_offset: u64,
        bytes: &[u8],
        one_shot: bool,
    ) -> Result<(), Refused> {
        let size = bytes.len() as u64;
        let offset = self.reserve(device, size, wgpu::MAP_ALIGNMENT, one_shot);
        let chunk = self.active.last().expect("the reserved chunk is active");
        let slice = chunk.buffer.slice(offset..offset + size);
        match slice.get_mapped_range_mut() {
            Ok(mut mapped) => mapped.copy_from_slice(bytes),
            Err(error) => return Err(self.refuse(error)),
        }
        encoder.copy_buffer_to_buffer(&chunk.buffer, offset, target, target_offset, size);
        Ok(())
    }

    /// Records a copy of the rectangle of `write`, whose rows fit one chunk, from rows that start
    /// `stride` bytes apart in `bytes`.
    fn write_band(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        write: &TexelWrite<'_>,
        stride: u64,
        bytes: &[u8],
        one_shot: bool,
    ) -> Result<(), Refused> {
        let (width, height) = write.extent;
        let row = u64::from(width) * u64::from(write.texel);
        let padded = padded_row(row);
        let staged = padded * u64::from(height);

        let alignment = u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let offset = self.reserve(device, staged, alignment, one_shot);
        let chunk = self.active.last().expect("the reserved chunk is active");
        let slice = chunk.buffer.slice(offset..offset + staged);
        {
            let mut mapped = match slice.get_mapped_range_mut() {
                Ok(mapped) => mapped,
                Err(error) => return Err(self.refuse(error)),
            };
            for y in 0..height as usize {
                let source = y * stride as usize;
                let target = y * padded as usize;
                mapped
                    .slice(target..target + row as usize)
                    .copy_from_slice(&bytes[source..source + row as usize]);
            }
        }
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &chunk.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset,
                    bytes_per_row: Some(padded as u32),
                    rows_per_image: Some(height),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: write.target,
                mip_level: write.mip,
                origin: wgpu::Origin3d {
                    x: write.origin.0,
                    y: write.origin.1,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }
}

/// Returns the staged length of a texel row of `row` bytes.
fn padded_row(row: u64) -> u64 {
    row.next_multiple_of(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
}
