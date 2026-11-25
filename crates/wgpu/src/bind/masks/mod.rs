//! Copies of the atlas tiles mask clips sample, gathered into two textures every pipeline binds.
//!
//! Each mask is copied into the alpha masks or the colour masks, by the format of its page, and
//! its link is pointed at the copy. A write to a page copies its masks again. A mask texture that
//! runs out of room is replaced by one at least twice as large, and its masks are copied again.

mod store;

use iceblink::{PageFormat, PageOp};

use crate::bind::tables::{FrameTables, MaskRequest};
use crate::gpu::device::Gpu;
use crate::pages::Pages;

use self::store::Store;

/// A mask copied into a mask texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Placed {
    request: MaskRequest,
    /// Whether the copy is in the colour masks.
    color: bool,
    /// The copy's top-left texel.
    cell: [u32; 2],
}

/// The two mask textures and the masks placed in them.
#[derive(Debug)]
pub(crate) struct ClipMasks {
    alpha: Store,
    color: Store,
    placed: Vec<Placed>,
    generation: u64,
    requests: Vec<MaskRequest>,
}

impl ClipMasks {
    pub(crate) fn new(gpu: &Gpu) -> Self {
        Self {
            alpha: Store::new(gpu, wgpu::TextureFormat::R8Unorm, 1),
            color: Store::new(gpu, wgpu::TextureFormat::Rgba8Unorm, 1),
            placed: Vec::new(),
            generation: 1,
            requests: Vec::new(),
        }
    }

    pub(crate) fn alpha_view(&self) -> &wgpu::TextureView {
        &self.alpha.view
    }

    pub(crate) fn color_view(&self) -> &wgpu::TextureView {
        &self.color.view
    }

    /// Returns a number that changes whenever a mask texture is replaced.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.alpha.bytes() + self.color.bytes()
    }

    /// Places the masks the tables resolved since the last update, and copies the masks of the
    /// pages `ops` writes again.
    pub(crate) fn update(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        tables: &mut FrameTables,
        pages: &Pages,
        ops: &[PageOp],
    ) {
        tables.take_mask_requests(&mut self.requests);
        let written = |page: u32| {
            ops.iter().any(|op| match op {
                // Masks are copied from level 0.
                PageOp::Upload { level, .. } if *level > 0 => false,
                PageOp::Create { page: held, .. }
                | PageOp::Upload { page: held, .. }
                | PageOp::Destroy { page: held } => *held == page,
            })
        };
        if !ops.is_empty() {
            for placed in &self.placed {
                if written(placed.request.page) {
                    copy(encoder, pages, placed, self.store(placed.color));
                }
            }
        }
        let requests = std::mem::take(&mut self.requests);
        for request in &requests {
            self.place(gpu, encoder, tables, pages, *request);
        }
        self.requests = requests;
        self.requests.clear();
    }

    fn store(&self, color: bool) -> &Store {
        if color { &self.color } else { &self.alpha }
    }

    /// Copies the mask of `request` into its mask texture, and points its link at the copy.
    ///
    /// A mask that finds no room in a texture of the device's largest size is left out.
    fn place(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        tables: &mut FrameTables,
        pages: &Pages,
        request: MaskRequest,
    ) {
        let Some(page) = pages.get(request.page) else {
            return;
        };
        let color = page.format != PageFormat::A8;
        let size = [request.rect[2].max(1), request.rect[3].max(1)];
        let limit = gpu.device().limits().max_texture_dimension_2d;
        let cell = loop {
            let store = if color {
                &mut self.color
            } else {
                &mut self.alpha
            };
            if let Some(cell) = store.place(size) {
                break cell;
            }
            // A mask that does not fit the largest texture is left uncovered.
            if store.side >= limit {
                tracing::warn!(?size, "a clip mask does not fit the mask texture");
                return;
            }
            self.grow(gpu, encoder, tables, pages, color, size);
        };
        let placed = Placed {
            request,
            color,
            cell,
        };
        copy(encoder, pages, &placed, self.store(color));
        tables.place_mask(
            request.link,
            [
                cell[0] as f32,
                cell[1] as f32,
                size[0] as f32,
                size[1] as f32,
            ],
            color,
        );
        self.placed.retain(|held| held.request.link != request.link);
        self.placed.push(placed);
    }

    /// Replaces the colour or alpha masks with a texture at least twice as large and large enough
    /// for `size`, and places their masks again.
    fn grow(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        tables: &mut FrameTables,
        pages: &Pages,
        color: bool,
        size: [u32; 2],
    ) {
        let held = self.store(color);
        let limit = gpu.device().limits().max_texture_dimension_2d;
        let mut side = (held.side * 2).max(256);
        while side < size[0].max(size[1]) {
            side *= 2;
        }
        let side = side.min(limit);
        let replacement = Store::new(gpu, held.format, side);
        if color {
            self.color = replacement;
        } else {
            self.alpha = replacement;
        }
        self.generation += 1;
        let moved: Vec<Placed> = self
            .placed
            .iter()
            .filter(|placed| placed.color == color)
            .copied()
            .collect();
        self.placed.retain(|placed| placed.color != color);
        for placed in moved {
            self.place(gpu, encoder, tables, pages, placed.request);
        }
    }
}

/// Records the copy of a placed mask from its page into `store`.
///
/// Nothing is copied when the page is gone or holds another format.
fn copy(encoder: &mut wgpu::CommandEncoder, pages: &Pages, placed: &Placed, store: &Store) {
    let Some(page) = pages.get(placed.request.page) else {
        return;
    };
    let [x, y, width, height] = placed.request.rect;
    let width = width.min(page.texture.width().saturating_sub(x));
    let height = height.min(page.texture.height().saturating_sub(y));
    if width == 0 || height == 0 || page.texture.format() != store.format {
        return;
    }
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &page.texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: &store.texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: placed.cell[0],
                y: placed.cell[1],
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
}
