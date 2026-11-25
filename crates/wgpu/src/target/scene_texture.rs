//! The kept target frames are composed into.
//!
//! Its allocation is rounded up to size classes, so a small resize keeps it. A resize past the
//! allocation carries the old picture into the new texture at the start of the next frame.

use iceblink::geom::{Device, Point, Rect, Size};

use crate::gpu::device::Gpu;

/// The granularity allocations are rounded up to, in device pixels.
pub(crate) const SIZE_CLASS: i32 = 256;

/// How many times the area a resize needs the allocation may hold before the resize shrinks it.
pub(crate) const SHRINK_SLACK: i64 = 2;

/// Returns `length` rounded up to a whole number of size classes, and at least one class.
pub(crate) fn size_class(length: i32) -> i32 {
    let length = length.max(1) as u32;
    (length.div_ceil(SIZE_CLASS as u32) * SIZE_CLASS as u32) as i32
}

/// The kept target, allocated in size classes.
#[derive(Debug)]
pub(crate) struct SceneTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    /// The part of the texture the target covers.
    used: Rect<i32, Device>,
    /// The replaced texture whose picture is still to be copied in.
    carried: Option<Carried>,
}

/// A replaced texture and the part of it that holds the picture.
#[derive(Debug)]
struct Carried {
    texture: wgpu::Texture,
    held: Rect<i32, Device>,
}

impl SceneTexture {
    const USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT
        .union(wgpu::TextureUsages::TEXTURE_BINDING)
        .union(wgpu::TextureUsages::COPY_SRC)
        .union(wgpu::TextureUsages::COPY_DST);

    /// Returns a target of `size` device pixels in `format`, which is not an sRGB format.
    pub(crate) fn new(gpu: &Gpu, size: Size<i32, Device>, format: wgpu::TextureFormat) -> Self {
        debug_assert!(
            !format.is_srgb(),
            "the composed target must not encode: {format:?}"
        );
        let allocated: Size<i32, Device> =
            Size::new(size_class(size.width), size_class(size.height));
        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("iceblink.scene"),
            size: wgpu::Extent3d {
                width: allocated.width as u32,
                height: allocated.height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: Self::USAGE,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            format,
            used: Rect::new(Point::new(0, 0), size.non_negative()),
            carried: None,
        }
    }

    /// Fits the target to `size`, and reports whether the texture was replaced.
    ///
    /// The texture is replaced when `size` does not fit it, or when it holds more than
    /// [`SHRINK_SLACK`] times the area `size` needs. The picture of a replaced texture is kept for
    /// [`carry`](Self::carry).
    pub(crate) fn resize(&mut self, gpu: &Gpu, size: Size<i32, Device>) -> bool {
        let wanted: Size<i32, Device> = Size::new(size_class(size.width), size_class(size.height));
        let allocated = self.allocated();
        let fits = size.width <= allocated.width && size.height <= allocated.height;
        let area = |extent: Size<i32, Device>| extent.width as i64 * extent.height as i64;
        if fits && area(allocated) <= SHRINK_SLACK * area(wanted) {
            self.used = Rect::new(Point::new(0, 0), size.non_negative());
            return false;
        }
        let replaced = std::mem::replace(self, Self::new(gpu, size, self.format));
        // Two reallocations before a draw: the first replacement was never drawn into, so the
        // picture is still in the texture the first one replaced.
        self.carried = Some(replaced.carried.unwrap_or(Carried {
            texture: replaced.texture,
            held: replaced.used,
        }));
        true
    }

    /// Records the copy of the picture of a replaced texture into this one, and returns the part
    /// copied.
    ///
    /// Returns `None` when no picture is to be copied or none of it lies in the texture.
    pub(crate) fn carry(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Option<Rect<i32, Device>> {
        let carried = self.carried.take()?;
        let held = carried
            .held
            .intersection(Rect::new(Point::new(0, 0), self.allocated()))?;
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &carried.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: held.size.width as u32,
                height: held.size.height as u32,
                depth_or_array_layers: 1,
            },
        );
        Some(held)
    }

    /// Reports whether the picture of a replaced texture is still to be copied in.
    pub(crate) fn carrying(&self) -> bool {
        self.carried.is_some()
    }

    pub(crate) fn allocated(&self) -> Size<i32, Device> {
        Size::new(self.texture.width() as i32, self.texture.height() as i32)
    }

    /// Returns the part of the texture the target covers.
    pub(crate) fn used(&self) -> Rect<i32, Device> {
        self.used
    }

    pub(crate) fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub(crate) fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub(crate) fn bytes(&self) -> u64 {
        let allocated = self.allocated();
        let per_texel = self.format.block_copy_size(None).unwrap_or(4) as u64;
        per_texel * allocated.width.max(0) as u64 * allocated.height.max(0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::{SIZE_CLASS, size_class};

    #[test]
    fn a_size_rounds_up_to_its_class_and_never_to_nothing() {
        assert_eq!(size_class(1), SIZE_CLASS);
        assert_eq!(size_class(0), SIZE_CLASS);
        assert_eq!(size_class(-4), SIZE_CLASS);
        assert_eq!(size_class(SIZE_CLASS), SIZE_CLASS);
        assert_eq!(size_class(SIZE_CLASS + 1), 2 * SIZE_CLASS);
        assert_eq!(size_class(1920), 2048);
        assert_eq!(size_class(1080), 1280);
    }

    #[test]
    fn a_drag_across_one_class_costs_one_reallocation_and_not_one_per_frame() {
        // The whole point of the class: consecutive resize events inside a band agree.
        let band: Vec<i32> = (1793..=2048).map(size_class).collect();
        assert!(band.iter().all(|allocated| *allocated == 2048));
        assert_ne!(size_class(1792), size_class(1793));
    }
}
