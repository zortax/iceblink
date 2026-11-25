//! Moving kept pixels by a shift before the frame redraws its damage.

use iceblink::Shift;
use iceblink::geom::{Device, Rect};

use crate::gpu::device::Gpu;
use crate::target::scene_texture::size_class;

/// The texture a shift moves pixels through.
#[derive(Debug)]
pub(crate) struct ShiftScratch {
    texture: wgpu::Texture,
    allocated: (u32, u32),
    format: wgpu::TextureFormat,
}

impl ShiftScratch {
    const USAGE: wgpu::TextureUsages =
        wgpu::TextureUsages::COPY_SRC.union(wgpu::TextureUsages::COPY_DST);

    fn new(gpu: &Gpu, size: (u32, u32), format: wgpu::TextureFormat) -> Self {
        let allocated = (
            size_class(size.0 as i32).max(1) as u32,
            size_class(size.1 as i32).max(1) as u32,
        );
        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("iceblink.shift.scratch"),
            size: wgpu::Extent3d {
                width: allocated.0,
                height: allocated.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: Self::USAGE,
            view_formats: &[],
        });
        Self {
            texture,
            allocated,
            format,
        }
    }

    /// Returns the bytes of device memory the texture holds.
    pub(crate) fn bytes(&self) -> u64 {
        let block = self.format.target_pixel_byte_cost().unwrap_or(4) as u64;
        u64::from(self.allocated.0) * u64::from(self.allocated.1) * block
    }
}

/// Moves the pixels of `shift` within `composed`, through a scratch texture so source and
/// destination may overlap.
///
/// Returns whether anything moved.
pub(crate) fn apply(
    gpu: &Gpu,
    encoder: &mut wgpu::CommandEncoder,
    composed: &wgpu::Texture,
    scratch: &mut Option<ShiftScratch>,
    shift: Shift,
) -> bool {
    let (Some(from), Some(to)) = (shift.source(), shift.destination()) else {
        return false;
    };
    debug_assert_eq!(
        from.size, to.size,
        "a shift reads exactly what it writes, or it is not a translation",
    );
    let extent = wgpu::Extent3d {
        width: from.size.width.max(0) as u32,
        height: from.size.height.max(0) as u32,
        depth_or_array_layers: 1,
    };
    if extent.width == 0 || extent.height == 0 {
        return false;
    }

    let format = composed.format();
    let wanted = (extent.width, extent.height);
    let fits = scratch.as_ref().is_some_and(|held| {
        held.format == format && held.allocated.0 >= wanted.0 && held.allocated.1 >= wanted.1
    });
    if !fits {
        *scratch = Some(ShiftScratch::new(gpu, wanted, format));
    }
    let staged = scratch.as_ref().expect("just allocated");

    encoder.copy_texture_to_texture(
        texel_copy(composed, from),
        wgpu::TexelCopyTextureInfo {
            texture: &staged.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        extent,
    );
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &staged.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        texel_copy(composed, to),
        extent,
    );
    true
}

fn texel_copy(texture: &wgpu::Texture, at: Rect<i32, Device>) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d {
            x: at.origin.x.max(0) as u32,
            y: at.origin.y.max(0) as u32,
            z: 0,
        },
        aspect: wgpu::TextureAspect::All,
    }
}
