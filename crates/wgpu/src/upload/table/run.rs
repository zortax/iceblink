//! Runs of texels written into a table texture.

use crate::upload::belt::{Refused, TexelWrite, UploadBelt};
use crate::upload::table::rows::{TEXEL, TEXELS_WIDE};

/// Records a copy of the texels `bytes` into `texture`, starting at texel `first` in row order,
/// and returns the bytes written.
///
/// The run is cut to the texels the texture holds.
///
/// # Errors
///
/// Returns [`Refused`] when the device refuses a staging mapping.
pub fn write_run(
    device: &wgpu::Device,
    belt: &mut UploadBelt,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    first: u32,
    bytes: &[u8],
) -> Result<u64, Refused> {
    debug_assert_eq!(bytes.len() % TEXEL, 0, "a run is a whole number of texels");
    // A table at the row limit of the device holds less than it was asked to, and a copy past
    // its last row is one the device refuses.
    let held = u64::from(texture.height()) * u64::from(TEXELS_WIDE);
    let room = held.saturating_sub(u64::from(first)) as usize * TEXEL;
    let mut texel = first;
    let mut rest = &bytes[..bytes.len().min(room)];
    let mut written = 0;

    while !rest.is_empty() {
        let column = texel % TEXELS_WIDE;
        let row = texel / TEXELS_WIDE;
        let (width, height) = run_extent(texel, (rest.len() / TEXEL) as u32);
        let taken = width as usize * height as usize * TEXEL;

        let write = TexelWrite {
            target: texture,
            mip: 0,
            origin: (column, row),
            extent: (width, height),
            texel: TEXEL as u32,
        };
        written += belt.write_texels(device, encoder, &write, &rest[..taken])?;

        texel += width * height;
        rest = &rest[taken..];
    }
    Ok(written)
}

/// Returns the extent of the next rectangle of a run at `texel` with `held` texels left: whole
/// rows from the start of a row, and otherwise the rest of the row.
fn run_extent(texel: u32, held: u32) -> (u32, u32) {
    let column = texel % TEXELS_WIDE;
    if column == 0 && held >= TEXELS_WIDE {
        (TEXELS_WIDE, held / TEXELS_WIDE)
    } else {
        ((TEXELS_WIDE - column).min(held), 1)
    }
}
