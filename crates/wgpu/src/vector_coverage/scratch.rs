//! The two textures a pass is drawn through.

use crate::vector::{Decay, Extent, VectorTarget};

/// The format of both textures.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// An accumulation texture and a straight texture per layer, with room for one round of passes.
///
/// Outlines accumulate premultiplied into the first, and one draw per layer converts the layer
/// into straight colour in the second, which composites read. The allocation follows [`Decay`].
#[derive(Debug)]
pub(crate) struct Scratch {
    /// Where outlines accumulate, premultiplied.
    accumulation: Option<Textures>,
    /// What composites read, straight.
    straight: Option<Textures>,
    /// The extent of every layer.
    extent: (u32, u32),
    /// How many layers are allocated.
    layers: u32,
    /// The allocation policy.
    decay: Decay,
}

/// One texture per layer and its view.
// A texture per layer instead of one array texture: rendering into a layer of an array texture
// writes nothing on an OpenGL 3.3 device, which is the device this rasteriser is for.
#[derive(Debug)]
struct Textures {
    textures: Vec<wgpu::Texture>,
    /// One view per layer.
    views: Vec<wgpu::TextureView>,
}

impl Scratch {
    /// The fewest layers a scratch shrinks to.
    pub(crate) const LAYERS: u32 = 1;

    /// The most layers one round uses.
    pub(crate) const MAX_LAYERS: u32 = 64;

    /// Returns a scratch with nothing allocated.
    pub(crate) fn new() -> Self {
        Self {
            accumulation: None,
            straight: None,
            extent: (0, 0),
            layers: 0,
            decay: Decay::new(),
        }
    }

    /// Returns how many layers are allocated.
    pub(crate) fn layers(&self) -> u32 {
        self.layers
    }

    /// Makes room for `layers` layers of at least `width` by `height`.
    ///
    /// The layers are clamped to between [`LAYERS`](Self::LAYERS) and
    /// [`MAX_LAYERS`](Self::MAX_LAYERS).
    pub(crate) fn ensure(&mut self, device: &wgpu::Device, width: u32, height: u32, layers: u32) {
        let want = Extent::new(
            width.max(1),
            height.max(1),
            layers.clamp(Self::LAYERS, Self::MAX_LAYERS),
        );
        let Some(extent) = self.decay.wants(want) else {
            return;
        };
        let size = (extent.width.max(1), extent.height.max(1));
        let depth = extent.layers.clamp(Self::LAYERS, Self::MAX_LAYERS);
        self.accumulation = Some(allocate(
            device,
            size,
            depth,
            "iceblink.vector.coverage.accumulation",
        ));
        self.straight = Some(allocate(
            device,
            size,
            depth,
            "iceblink.vector.coverage.straight",
        ));
        self.extent = size;
        self.layers = depth;
    }

    /// Returns the view outlines of `layer` accumulate into.
    pub(crate) fn accumulation(&self, layer: u32) -> Option<&wgpu::TextureView> {
        self.accumulation
            .as_ref()
            .and_then(|held| held.views.get(layer as usize))
    }

    /// Returns the view composites of `layer` read.
    pub(crate) fn straight(&self, layer: u32) -> Option<&wgpu::TextureView> {
        self.straight
            .as_ref()
            .and_then(|held| held.views.get(layer as usize))
    }

    /// Returns the texture composites of `layer` read.
    pub(crate) fn straight_texture(&self, layer: u32) -> Option<&wgpu::Texture> {
        self.straight
            .as_ref()
            .and_then(|held| held.textures.get(layer as usize))
    }

    /// Returns the extent of every layer.
    pub(crate) fn extent(&self) -> (u32, u32) {
        self.extent
    }

    /// Returns the bytes of device memory both textures hold.
    pub(crate) fn bytes(&self) -> u64 {
        let one = u64::from(self.extent.0) * u64::from(self.extent.1) * u64::from(self.layers) * 4;
        if self.accumulation.is_some() {
            2 * one
        } else {
            0
        }
    }

    /// Drops both textures, and returns the bytes freed.
    pub(crate) fn release(&mut self) -> u64 {
        let freed = self.bytes();
        self.accumulation = None;
        self.straight = None;
        self.extent = (0, 0);
        self.layers = 0;
        self.decay.clear();
        freed
    }

    /// Clears the layers of `targets` in both textures to transparent, in one submission.
    // Both textures, so a pass that fails part way leaves no earlier frame for a composite.
    pub(crate) fn clear(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        targets: &[VectorTarget],
    ) {
        if targets.is_empty() || self.accumulation.is_none() {
            return;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("iceblink.vector.coverage.clear"),
        });
        for layer in targets
            .iter()
            .filter_map(|target| u32::try_from(target.0).ok())
        {
            for view in [self.accumulation(layer), self.straight(layer)]
                .into_iter()
                .flatten()
            {
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("iceblink.vector.coverage.clear"),
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
        }
        queue.submit([encoder.finish()]);
    }
}

/// Allocates `layers` textures of `extent` and a view of each.
fn allocate(
    device: &wgpu::Device,
    extent: (u32, u32),
    layers: u32,
    label: &'static str,
) -> Textures {
    let textures: Vec<wgpu::Texture> = (0..layers)
        .map(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: extent.0,
                    height: extent.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        })
        .collect();
    let views = textures
        .iter()
        .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()))
        .collect();
    Textures { textures, views }
}
