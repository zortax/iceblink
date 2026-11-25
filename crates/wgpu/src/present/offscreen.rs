//! A presenter of textures that no window shows, for headless drawing and tests.

use std::any::Any;
use std::sync::Arc;

use iceblink::Presenter;
use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, BufferAge, GpuUnavailable, PresentCapabilities, PresentFeedback, PresentMode,
    PresentResult, STALE_DAMAGE, SkipReason,
};

use crate::gpu::device::Gpu;
use crate::gpu::formats::{Formats, SrgbTier};
use crate::present::WgpuPresenter;
use crate::present::acquisition::Acquisition;
use crate::present::debt::ImageDebts;
use crate::present::fault::FaultInjector;
use crate::present::surface::cropped_extent;

/// One image and the view it is written through.
#[derive(Debug)]
struct Image {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// The present count when the image was last presented.
    presented: Option<u64>,
}

/// A presenter of textures that no window shows.
///
/// In kept-target mode it holds one image that keeps its texels, so each copy covers only the
/// image's debt. In buffer-age mode it rotates through several images and reports each one's age,
/// and the renderer draws straight into the acquired image.
///
/// The images are made once at the size the presenter is built for. A later size takes images
/// rounded up to size classes, which a small change of size keeps, together with their texels.
#[derive(Debug)]
pub struct Offscreen {
    gpu: Arc<Gpu>,
    mode: PresentMode,
    count: usize,
    formats: Formats,
    /// The extent frames are presented at.
    size: [u32; 2],
    /// The extent of the images, which holds `size`.
    allocated: [u32; 2],
    images: Vec<Image>,
    debts: ImageDebts,
    next: usize,
    acquired: Option<u32>,
    presents: u64,
    last: Option<u32>,
    last_damage: DamageSet,
    faults: FaultInjector,
}

impl Offscreen {
    /// Returns a kept-target presenter of one image in `format`.
    pub fn kept(gpu: &Arc<Gpu>, size: [u32; 2], format: wgpu::TextureFormat) -> Self {
        Self::new(gpu, PresentMode::KeptTarget, 1, size, format)
    }

    /// Returns a buffer-age presenter rotating through `count` images in `format`.
    ///
    /// The images are drawn into, so `format` is an unencoded format.
    pub fn buffer_age(
        gpu: &Arc<Gpu>,
        count: usize,
        size: [u32; 2],
        format: wgpu::TextureFormat,
    ) -> Self {
        Self::new(gpu, PresentMode::BufferAge, count.max(1), size, format)
    }

    fn new(
        gpu: &Arc<Gpu>,
        mode: PresentMode,
        count: usize,
        size: [u32; 2],
        format: wgpu::TextureFormat,
    ) -> Self {
        let size = [size[0].max(1), size[1].max(1)];
        let mut offscreen = Self {
            gpu: Arc::clone(gpu),
            mode,
            count,
            formats: formats(format),
            size,
            allocated: [0, 0],
            images: Vec::new(),
            debts: ImageDebts::default(),
            next: 0,
            acquired: None,
            presents: 0,
            last: None,
            last_damage: DamageSet::new(),
            faults: FaultInjector::from_environment(),
        };
        offscreen.allocate(size);
        offscreen
    }

    /// Replaces the images with new ones of `size`, each owing the whole target.
    fn allocate(&mut self, size: [u32; 2]) {
        self.allocated = size;
        let view_formats = self.formats.view_formats();
        self.images = (0..self.count)
            .map(|_| {
                let texture = self.gpu.device().create_texture(&wgpu::TextureDescriptor {
                    label: Some("iceblink.offscreen"),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.formats.surface,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::COPY_DST,
                    view_formats: &view_formats,
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(self.formats.present_attachment()),
                    ..Default::default()
                });
                Image {
                    texture,
                    view,
                    presented: None,
                }
            })
            .collect();
        self.debts = ImageDebts::new(self.count);
        self.next = 0;
        self.acquired = None;
        self.last = None;
    }

    /// Returns the texture of `image`, whose texels from the origin up to the presenter's extent
    /// hold the presented frame.
    pub fn texture(&self, image: u32) -> Option<&wgpu::Texture> {
        self.images.get(image as usize).map(|image| &image.texture)
    }

    /// Returns the image presented last, if any.
    pub fn last_presented(&self) -> Option<u32> {
        self.last
    }

    /// Returns the damage of the last present.
    pub fn last_damage(&self) -> &DamageSet {
        &self.last_damage
    }

    /// Queues `answer` in place of the next `times` acquisitions.
    pub fn inject_fault(&mut self, answer: Acquisition, times: u32) {
        self.faults.inject(answer, times);
    }

    /// Returns the age `image` is acquired with.
    fn age_of(&self, image: usize) -> BufferAge {
        match (self.mode, self.images[image].presented) {
            (PresentMode::KeptTarget, _) => BufferAge::Kept,
            (PresentMode::BufferAge, Some(presented)) => {
                BufferAge::Known((self.presents - presented + 1) as u32)
            }
            (PresentMode::BufferAge, None) => BufferAge::Unknown,
        }
    }
}

/// Returns the formats of offscreen images in `format`.
fn formats(format: wgpu::TextureFormat) -> Formats {
    Formats {
        surface: format,
        scene: format.remove_srgb_suffix(),
        scratch: wgpu::TextureFormat::Rgba8Unorm,
        view_format_twin: format.is_srgb().then(|| format.remove_srgb_suffix()),
        tier: if format.is_srgb() {
            SrgbTier::ViewFormatTwin
        } else {
            SrgbTier::Native
        },
        alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
    }
}

impl Presenter for Offscreen {
    fn capabilities(&self) -> PresentCapabilities {
        PresentCapabilities {
            mode: self.mode,
            partial_present: true,
            feedback: false,
        }
    }

    fn configure(&mut self, size: [u32; 2], _opaque: bool) -> Result<(), SkipReason> {
        let largest = self.gpu.capabilities().max_texture_size;
        let size = [size[0].clamp(1, largest), size[1].clamp(1, largest)];
        if size == self.size {
            return Ok(());
        }
        self.size = size;
        let extent = cropped_extent(size, Some(self.allocated), largest);
        if extent != self.allocated {
            self.allocate(extent);
        }
        Ok(())
    }

    fn acquire(&mut self) -> Acquired {
        if let Some(image) = self.acquired {
            return Acquired::Image {
                image,
                age: self.age_of(image as usize),
            };
        }
        if let Some(answer) = self.faults.take()
            && !answer.presents()
        {
            return Acquired::Skipped(answer.skip());
        }
        let image = self.next;
        self.next = (self.next + 1) % self.images.len();
        self.acquired = Some(image as u32);
        Acquired::Image {
            image: image as u32,
            age: self.age_of(image),
        }
    }

    fn acquired(&self) -> Option<u32> {
        self.acquired
    }

    fn release(&mut self, image: u32) {
        if self.acquired == Some(image) {
            self.acquired = None;
        }
    }

    fn composed(&mut self, damage: &DamageSet) {
        if self.mode == PresentMode::KeptTarget {
            self.debts.composed(damage);
        }
    }

    fn debt(&self, image: u32) -> &DamageSet<STALE_DAMAGE> {
        static NONE: DamageSet<STALE_DAMAGE> = DamageSet::new();
        match self.mode {
            PresentMode::KeptTarget => self.debts.debt(image),
            PresentMode::BufferAge => &NONE,
        }
    }

    fn present(&mut self, image: u32, damage: &DamageSet) -> PresentResult {
        if self.acquired != Some(image) {
            return PresentResult::Skipped(SkipReason::Unacquired);
        }
        self.acquired = None;
        self.presents += 1;
        if let Some(held) = self.images.get_mut(image as usize) {
            held.presented = Some(self.presents);
        }
        self.debts.paid(image);
        self.last = Some(image);
        self.last_damage = *damage;
        PresentResult::Presented
    }

    fn next_deadline(&self) -> Option<u64> {
        None
    }

    fn feedback(&mut self, _out: &mut Vec<PresentFeedback>) {}

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

impl WgpuPresenter for Offscreen {
    fn formats(&self) -> Formats {
        self.formats
    }

    fn extent(&self) -> [u32; 2] {
        self.size
    }

    fn image_texture(&self, image: u32) -> Option<&wgpu::Texture> {
        self.texture(image)
    }

    fn image_view(&self, image: u32) -> Option<&wgpu::TextureView> {
        self.images.get(image as usize).map(|image| &image.view)
    }

    fn keeps_contents(&self) -> bool {
        true
    }

    fn rebuild(&mut self, gpu: &Arc<Gpu>) -> Result<(), GpuUnavailable> {
        self.gpu = Arc::clone(gpu);
        self.allocate(self.allocated);
        Ok(())
    }
}
