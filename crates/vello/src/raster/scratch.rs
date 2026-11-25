//! The texture a round of passes is rasterised into.

use iceblink_wgpu::vector::{Decay, Extent, VectorTarget};

/// The format the path renderer writes.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// An array texture with room for one round of passes, and a view per layer.
///
/// Each pass is rasterised into a bin of its own size in some layer. The allocation follows
/// [`Decay`], with the width and height rounded up to its size class.
#[derive(Debug)]
pub(crate) struct Scratch {
    /// The array texture, or nothing before the first pass.
    texture: Option<wgpu::Texture>,
    /// One view per layer.
    views: Vec<wgpu::TextureView>,
    /// The extent of every layer.
    extent: (u32, u32),
    /// The allocation policy.
    decay: Decay,
}

impl Scratch {
    /// The fewest layers a scratch shrinks to.
    pub(crate) const MIN_LAYERS: u32 = 1;

    /// The most layers one round uses.
    pub(crate) const MAX_LAYERS: u32 = 64;

    /// Returns a scratch with nothing allocated.
    pub(crate) fn new() -> Self {
        Self {
            texture: None,
            views: Vec::new(),
            extent: (0, 0),
            decay: Decay::new(),
        }
    }

    /// Returns how many layers are allocated.
    pub(crate) fn layers(&self) -> u32 {
        self.views.len() as u32
    }

    /// Makes room for `layers` layers of at least `width` by `height`.
    ///
    /// The layers are clamped to between [`MIN_LAYERS`](Self::MIN_LAYERS) and
    /// [`MAX_LAYERS`](Self::MAX_LAYERS).
    pub(crate) fn ensure(&mut self, device: &wgpu::Device, width: u32, height: u32, layers: u32) {
        let want = Extent::new(
            width.max(1),
            height.max(1),
            layers.clamp(Self::MIN_LAYERS, Self::MAX_LAYERS),
        )
        .classed();

        if let Some(extent) = self.decay.wants(want) {
            self.allocate(device, extent);
        }
    }

    /// Reallocates the texture and its views at `extent`.
    fn allocate(&mut self, device: &wgpu::Device, extent: Extent) {
        let size = (extent.width.max(1), extent.height.max(1));
        let layers = extent.layers.clamp(Self::MIN_LAYERS, Self::MAX_LAYERS);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iceblink.vector.scratch"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            // The path renderer writes storage, and the clear is a render pass.
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.views = (0..layers)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("iceblink.vector.scratch.layer"),
                    format: Some(FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    usage: None,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                })
            })
            .collect();
        self.texture = Some(texture);
        self.extent = size;
    }

    /// Returns the view of `layer`, or `None` for a layer that is not allocated.
    pub(crate) fn view(&self, layer: u32) -> Option<&wgpu::TextureView> {
        self.views.get(layer as usize)
    }

    /// Returns the texture and the array layer of `layer`, or `None` for a layer that is not
    /// allocated.
    pub(crate) fn texture(&self, layer: u32) -> Option<(&wgpu::Texture, u32)> {
        let texture = self.texture.as_ref()?;
        (layer < texture.depth_or_array_layers()).then_some((texture, layer))
    }

    /// Returns the extent of every layer.
    pub(crate) fn extent(&self) -> (u32, u32) {
        self.extent
    }

    /// Returns the bytes of device memory the texture holds.
    pub(crate) fn bytes(&self) -> u64 {
        self.texture.as_ref().map_or(0, |texture| {
            u64::from(texture.width())
                * u64::from(texture.height())
                * u64::from(texture.depth_or_array_layers())
                * 4
        })
    }

    /// Drops the texture, and returns the bytes freed.
    pub(crate) fn release(&mut self) -> u64 {
        let freed = self.bytes();
        self.texture = None;
        self.views = Vec::new();
        self.extent = (0, 0);
        self.decay.clear();
        freed
    }

    /// Clears the whole layers of `targets` to transparent, in one submission.
    // The whole layer, so a pass smaller than the last one in it leaves nothing of the last.
    pub(crate) fn clear(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        targets: &[VectorTarget],
    ) {
        if targets.is_empty() || self.views.is_empty() {
            return;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("iceblink.vector.clear"),
        });
        for view in targets
            .iter()
            .filter_map(|target| self.view(u32::try_from(target.0).ok()?))
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("iceblink.vector.clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        queue.submit([encoder.finish()]);
    }
}
