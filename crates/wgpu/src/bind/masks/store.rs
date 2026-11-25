//! A mask texture filled by shelf packing.

use crate::gpu::device::Gpu;

/// A square mask texture whose masks are placed on shelves.
#[derive(Debug)]
pub(super) struct Store {
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) format: wgpu::TextureFormat,
    /// Where the next mask goes on the open shelf.
    x: u32,
    /// The top of the open shelf.
    y: u32,
    /// The height of the open shelf.
    shelf: u32,
    /// The width and height of the texture.
    pub(super) side: u32,
}

impl Store {
    /// Returns an empty mask texture of `format`, `side` texels wide and tall.
    pub(super) fn new(gpu: &Gpu, format: wgpu::TextureFormat, side: u32) -> Self {
        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("iceblink.clip_masks"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            format,
            x: 0,
            y: 0,
            shelf: 0,
            side,
        }
    }

    /// Returns where a mask of `size` goes, or `None` if the texture has no room for it.
    pub(super) fn place(&mut self, size: [u32; 2]) -> Option<[u32; 2]> {
        if size[0] > self.side || size[1] > self.side {
            return None;
        }
        if self.x + size[0] > self.side {
            self.y += self.shelf;
            self.x = 0;
            self.shelf = 0;
        }
        if self.y + size[1] > self.side {
            return None;
        }
        let at = [self.x, self.y];
        self.x += size[0];
        self.shelf = self.shelf.max(size[1]);
        Some(at)
    }

    pub(super) fn bytes(&self) -> u64 {
        u64::from(self.side)
            * u64::from(self.side)
            * if self.format == wgpu::TextureFormat::R8Unorm {
                1
            } else {
                4
            }
    }
}
