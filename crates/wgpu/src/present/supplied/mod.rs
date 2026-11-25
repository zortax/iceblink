//! A presenter over textures the application owns and rotates between, such as a display's
//! scanout buffers.
//!
//! The application chooses the texture the next frame is copied into, and presents the textures
//! itself. Each texture keeps its texels, so a copy covers only its debt.

mod presenter;
mod refusal;

use std::sync::Arc;

use iceblink::damage::DamageSet;
use iceblink::{GpuUnavailable, STALE_DAMAGE};

use crate::gpu::formats::Formats;
use crate::present::debt::ImageDebts;
use crate::renderer::{SharedGraphics, WgpuRenderer};

/// A set of textures the application supplies, which frames are copied into in kept-target
/// mode.
///
/// Every texture of the set has one format and one extent, and the image id of a texture is its
/// slot in the set. The set presents only while the output is configured at the textures' extent.
///
/// A rotated texture owes everything composed since it was last copied into. A consumed set owes
/// everything composed since the last copy into any of its textures, for an application that
/// copies each texture out once and never reads it again.
#[derive(Debug)]
pub struct Supplied {
    textures: Vec<wgpu::Texture>,
    views: Vec<wgpu::TextureView>,
    /// The slot of the texture the next frame is copied into.
    selected: u32,
    size: [u32; 2],
    formats: Formats,
    debts: ImageDebts,
    /// The damage composed since the last copy into any texture.
    uncopied: DamageSet<STALE_DAMAGE>,
    /// Whether the application copies each texture out once and never reads it again.
    consumed: bool,
    /// Whether the output is configured at an extent other than the textures'.
    diverged: bool,
    acquired: Option<u32>,
    /// The refresh interval of the display the textures are shown on, as the application knows it.
    refresh: Option<u64>,
}

impl Supplied {
    /// Returns a presenter over `textures`, of which the first is written next.
    ///
    /// The extent and the formats are the textures' own. An encoded texture is written through
    /// its own format, with the encode cancelled in the copy.
    ///
    /// Fails with the reason [`Supplied::unusable`] gives.
    pub fn new(textures: Vec<wgpu::Texture>) -> Result<Self, String> {
        if let Some(reason) = Self::unusable(&textures) {
            return Err(reason);
        }
        let first = &textures[0];
        let size = [first.width(), first.height()];
        // A texture that exists cannot be given another view format.
        let formats = Formats::choose(
            &[first.format()],
            &[wgpu::CompositeAlphaMode::Opaque],
            true,
            false,
        );
        let views = textures
            .iter()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()))
            .collect();
        Ok(Self {
            debts: ImageDebts::new(textures.len()),
            uncopied: DamageSet::full(),
            views,
            textures,
            selected: 0,
            size,
            formats,
            consumed: false,
            diverged: false,
            acquired: None,
            refresh: None,
        })
    }

    /// Returns a renderer on the device of `graphics` that presents into `textures`.
    ///
    /// The textures must have been created on that device, so `graphics` must already have one,
    /// as [`SharedGraphics::open_gpu`] opens it.
    ///
    /// Fails when `graphics` has no usable device, or with the reason the textures are refused.
    pub fn renderer(
        graphics: &SharedGraphics,
        textures: Vec<wgpu::Texture>,
    ) -> Result<WgpuRenderer, GpuUnavailable> {
        let Some(gpu) = graphics.gpu().filter(|gpu| !gpu.loss().is_lost()) else {
            return Err(GpuUnavailable::default().rejected(
                "the shared device",
                "no device is open, and supplied textures are made on the device \
                 SharedGraphics::open_gpu returns",
            ));
        };
        let mut supplied = Some(
            Self::new(textures)
                .map_err(|reason| GpuUnavailable::default().rejected(gpu.describe(), reason))?,
        );
        graphics.renderer_with_presenter(move |opened: &Arc<crate::gpu::device::Gpu>| {
            if !Arc::ptr_eq(opened, &gpu) {
                return Err("the supplied textures were made on another device".to_owned());
            }
            supplied
                .take()
                .ok_or_else(|| "the supplied textures are presented once".to_owned())
        })
    }

    /// Points the next frame at the texture at `slot`, and reports whether the slot exists.
    ///
    /// A slot outside the set leaves the selection unchanged.
    #[must_use = "a refused slot leaves the next frame going where it was going"]
    pub fn select(&mut self, slot: u32) -> bool {
        if slot as usize >= self.textures.len() {
            tracing::warn!(
                slot,
                supplied = self.textures.len(),
                selected = self.selected,
                "a slot outside the supplied textures was asked for, so the selection stands"
            );
            return false;
        }
        self.selected = slot;
        true
    }

    /// Returns the slot of the texture the next frame is copied into.
    pub fn selected(&self) -> u32 {
        self.selected
    }

    /// Sets the refresh interval of the display the textures are shown on, or `None` when it is
    /// not known.
    pub fn set_refresh(&mut self, refresh_ns: Option<u64>) {
        self.refresh = refresh_ns;
    }

    /// Sets whether the application copies each texture out once and never reads it again.
    pub fn set_consumed(&mut self, consumed: bool) {
        self.consumed = consumed;
    }

    /// Returns how many textures the set holds, which is at least one.
    #[expect(
        clippy::len_without_is_empty,
        reason = "a set is never empty, since an empty one is refused"
    )]
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Returns the extent of every texture in the set.
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// Returns the texture at `slot`.
    pub fn texture(&self, slot: u32) -> Option<&wgpu::Texture> {
        self.textures.get(slot as usize)
    }

    /// Returns the formats derived from the textures.
    pub fn formats(&self) -> Formats {
        self.formats
    }
}
