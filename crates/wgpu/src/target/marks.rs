//! The bin pages union marks write their coverage into.

use iceblink::geom::{Device, Size};

use crate::gpu::device::Gpu;
use crate::pipeline::kind::PipelineKind;
use crate::vector::{Decay, Extent};

/// The bin pages, as layers of one texture sized by the decay policy.
#[derive(Debug, Default)]
pub(crate) struct MarksScratch {
    texture: Option<wgpu::Texture>,
    layers: Vec<wgpu::TextureView>,
    array: Option<wgpu::TextureView>,
    extent: Size<i32, Device>,
    decay: Decay,
    generation: u64,
}

/// Returns how many layers the texture of `layers` bin pages has.
// GL makes a texture of one layer a plain 2D texture, which has no array view.
fn allocated_layers(gpu: &Gpu, layers: u32) -> u32 {
    if gpu.adapter().get_info().backend == wgpu::Backend::Gl {
        layers.max(2)
    } else {
        layers
    }
}

impl MarksScratch {
    /// Makes room for `pages` bin pages of at least `extent`, rounded up to size classes.
    ///
    /// The texture follows the demand down after the decay policy's grace period, and is dropped
    /// once no page is asked for.
    pub(crate) fn ensure(&mut self, gpu: &Gpu, extent: Size<i32, Device>, pages: u32) {
        let want = if pages == 0 {
            Extent::NONE
        } else {
            Extent::new(
                extent.width.max(1) as u32,
                extent.height.max(1) as u32,
                pages,
            )
            .classed()
        };
        let Some(held) = self.decay.wants(want) else {
            return;
        };
        if held.layers == 0 {
            self.texture = None;
            self.layers.clear();
            self.array = None;
            self.extent = Size::new(0, 0);
            self.generation = self.generation.wrapping_add(1);
            return;
        }
        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("iceblink.marks.bins"),
            size: wgpu::Extent3d {
                width: held.width,
                height: held.height,
                depth_or_array_layers: allocated_layers(gpu, held.layers),
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PipelineKind::COVERAGE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.layers = (0..held.layers)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("iceblink.marks.bins.page"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        self.array = Some(texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("iceblink.marks.bins.array"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        }));
        self.texture = Some(texture);
        self.extent = Size::new(held.width as i32, held.height as i32);
        self.generation = self.generation.wrapping_add(1);
    }

    /// Returns a number that changes whenever the texture is replaced or dropped.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the view of bin page `page`.
    pub(crate) fn page(&self, page: u32) -> Option<&wgpu::TextureView> {
        self.layers.get(page as usize)
    }

    /// Returns the view of every bin page as an array.
    pub(crate) fn array(&self) -> Option<&wgpu::TextureView> {
        self.array.as_ref()
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.texture.as_ref().map_or(0, |texture| {
            u64::from(texture.width())
                * u64::from(texture.height())
                * u64::from(texture.depth_or_array_layers())
                * u64::from(texture.format().block_copy_size(None).unwrap_or(1))
        })
    }

    /// Drops the texture, and returns the bytes freed.
    pub(crate) fn release(&mut self) -> u64 {
        let freed = self.bytes();
        let generation = self.generation.wrapping_add(1);
        *self = Self::default();
        self.generation = generation;
        freed
    }
}
