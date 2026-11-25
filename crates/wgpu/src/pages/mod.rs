//! The atlas pages a frame's page work creates, writes and destroys.
//!
//! Coverage pages are sampled with the nearest texel, and colour pages with a filtering sampler
//! that blends between levels of detail.

use iceblink::{PageFormat, PageOp};

use crate::gpu::device::Gpu;
use crate::upload::{TexelWrite, UploadBelt};

/// One atlas page.
#[derive(Debug)]
pub(crate) struct Page {
    pub(crate) texture: wgpu::Texture,
    pub(crate) format: PageFormat,
    bind_group: wgpu::BindGroup,
    /// The bytes the page's levels hold.
    bytes: u64,
}

/// The atlas pages, indexed by page number.
#[derive(Debug)]
pub(crate) struct Pages {
    pages: Vec<Option<Page>>,
    nearest: wgpu::Sampler,
    filtering: wgpu::Sampler,
    layout: wgpu::BindGroupLayout,
    /// How many page writes were applied.
    writes: u64,
}

impl Pages {
    /// Returns an empty page set whose pages are bound with `layout`.
    pub(crate) fn new(gpu: &Gpu, layout: wgpu::BindGroupLayout) -> Self {
        let sampler = |label, filter, mipmap_filter| {
            gpu.device().create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter,
                ..Default::default()
            })
        };
        Self {
            pages: Vec::new(),
            nearest: sampler(
                "iceblink.page.sampler",
                wgpu::FilterMode::Nearest,
                wgpu::MipmapFilterMode::Nearest,
            ),
            filtering: sampler(
                "iceblink.page.sampler.filtering",
                wgpu::FilterMode::Linear,
                wgpu::MipmapFilterMode::Linear,
            ),
            layout,
            writes: 0,
        }
    }

    /// Applies `ops` in order and returns the bytes uploaded.
    ///
    /// A write to a page or level that was not created, or with fewer bytes than its rectangle
    /// needs, is dropped.
    pub(crate) fn apply(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        ops: &[PageOp],
    ) -> u64 {
        let mut uploaded = 0;
        for op in ops {
            match op {
                PageOp::Create {
                    page,
                    format,
                    size,
                    levels,
                } => self.create(gpu, *page, *format, *size, *levels),
                PageOp::Upload { .. } => uploaded += self.upload(gpu, belt, encoder, op),
                PageOp::Destroy { page } => {
                    if let Some(slot) = self.pages.get_mut(*page as usize) {
                        *slot = None;
                    }
                }
            }
        }
        uploaded
    }

    /// Returns page `page`, or `None` if it was not created or was destroyed.
    pub(crate) fn get(&self, page: u32) -> Option<&Page> {
        self.pages.get(page as usize).and_then(Option::as_ref)
    }

    pub(crate) fn bind_group(&self, page: u32) -> Option<&wgpu::BindGroup> {
        self.get(page).map(|page| &page.bind_group)
    }

    /// Returns the bytes the pages hold.
    pub(crate) fn bytes(&self) -> u64 {
        self.pages.iter().flatten().map(|page| page.bytes).sum()
    }

    /// Returns how many page writes were applied since the pages were made.
    pub(crate) fn writes(&self) -> u64 {
        self.writes
    }

    /// Records the copy of an upload `op`, and returns the bytes uploaded.
    fn upload(
        &mut self,
        gpu: &Gpu,
        belt: &mut UploadBelt,
        encoder: &mut wgpu::CommandEncoder,
        op: &PageOp,
    ) -> u64 {
        let PageOp::Upload {
            page,
            level,
            rect,
            bytes,
            offset,
            stride,
        } = op
        else {
            return 0;
        };
        let Some(held) = self.get(*page) else {
            tracing::warn!(page, "a write names a page that was not created");
            return 0;
        };
        if *level >= held.texture.mip_level_count() {
            tracing::warn!(page, level, "a write names a level the page lacks");
            return 0;
        }
        let Some(source) = bytes.get(*offset..) else {
            return 0;
        };

        let write = TexelWrite {
            target: &held.texture,
            mip: *level,
            origin: (rect.origin.x.max(0) as u32, rect.origin.y.max(0) as u32),
            extent: (
                rect.size.width.max(0) as u32,
                rect.size.height.max(0) as u32,
            ),
            texel: bytes_per_texel(held.format),
        };
        let written = belt.write_rows(gpu.device(), encoder, &write, u64::from(*stride), source);
        self.writes += 1;
        gpu.uploaded(written)
    }

    /// Creates page `page` with `levels` levels, clamped to the levels its size has, in place of
    /// any page of that number.
    fn create(&mut self, gpu: &Gpu, page: u32, format: PageFormat, size: [u32; 2], levels: u32) {
        let extent = wgpu::Extent3d {
            width: size[0].max(1),
            height: size[1].max(1),
            depth_or_array_layers: 1,
        };
        let levels = levels.clamp(1, extent.max_mips(wgpu::TextureDimension::D2));
        // A new page holds transparent texels.
        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("iceblink.page"),
            size: extent,
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format(format),
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = if format == PageFormat::RGBA8_PREMULTIPLIED {
            &self.filtering
        } else {
            &self.nearest
        };
        let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("iceblink.page"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        let at = page as usize;
        if self.pages.len() <= at {
            self.pages.resize_with(at + 1, || None);
        }
        self.pages[at] = Some(Page {
            texture,
            format,
            bind_group,
            bytes: (0..levels)
                .map(|level| {
                    let [width, height] = PageOp::level_size([extent.width, extent.height], level);
                    u64::from(width) * u64::from(height) * u64::from(bytes_per_texel(format))
                })
                .sum(),
        });
    }
}

/// Returns the texture format a page of `format` is held in.
pub(crate) fn texture_format(format: PageFormat) -> wgpu::TextureFormat {
    if format == PageFormat::A8 {
        wgpu::TextureFormat::R8Unorm
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    }
}

/// Returns the bytes of one texel of a page of `format`.
fn bytes_per_texel(format: PageFormat) -> u32 {
    if format == PageFormat::A8 { 1 } else { 4 }
}
