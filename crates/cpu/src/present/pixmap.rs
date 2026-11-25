//! A presenter of pixmaps that no window shows.

use std::any::Any;

use iceblink::Presenter;
use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, BufferAge, PresentCapabilities, PresentFeedback, PresentMode, PresentResult,
    STALE_DAMAGE, SkipReason,
};

use crate::present::{CpuPresenter, ImageDebts};
use crate::target::Pixmap;

#[derive(Debug)]
struct Image {
    pixels: Pixmap,
    // The present count when the image was last presented.
    presented: Option<u64>,
}

/// A presenter of pixmaps that no window shows, for headless drawing and tests.
///
/// In kept-target mode it holds one image that keeps its texels, so each copy covers only the
/// image's debt. In buffer-age mode it rotates through several images and reports each one's age,
/// and the renderer draws straight into the acquired image.
#[derive(Debug)]
pub struct PixmapPresenter {
    mode: PresentMode,
    count: usize,
    size: [u32; 2],
    images: Vec<Image>,
    debts: ImageDebts,
    next: usize,
    acquired: Option<u32>,
    presents: u64,
    last: Option<u32>,
    last_damage: DamageSet,
}

impl PixmapPresenter {
    /// Returns a kept-target presenter of one image of `size`.
    pub fn kept(size: [u32; 2]) -> Self {
        Self::new(PresentMode::KeptTarget, 1, size)
    }

    /// Returns a buffer-age presenter rotating through `count` images of `size`, at least one.
    pub fn buffer_age(count: usize, size: [u32; 2]) -> Self {
        Self::new(PresentMode::BufferAge, count.max(1), size)
    }

    fn new(mode: PresentMode, count: usize, size: [u32; 2]) -> Self {
        let mut presenter = Self {
            mode,
            count,
            size: [0, 0],
            images: Vec::new(),
            debts: ImageDebts::default(),
            next: 0,
            acquired: None,
            presents: 0,
            last: None,
            last_damage: DamageSet::new(),
        };
        presenter.allocate(size);
        presenter
    }

    fn allocate(&mut self, size: [u32; 2]) {
        let size = [size[0].max(1), size[1].max(1)];
        self.size = size;
        self.images = (0..self.count)
            .map(|_| Image {
                pixels: Pixmap::new(size[0], size[1]),
                presented: None,
            })
            .collect();
        self.debts = ImageDebts::new(self.count);
        self.next = 0;
        self.acquired = None;
        self.last = None;
    }

    /// Returns the image presented last, if any.
    pub fn last_presented(&self) -> Option<&Pixmap> {
        self.last
            .and_then(|image| self.images.get(image as usize))
            .map(|image| &image.pixels)
    }

    /// Returns the damage of the last present.
    pub fn last_damage(&self) -> &DamageSet {
        &self.last_damage
    }

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

impl Presenter for PixmapPresenter {
    fn capabilities(&self) -> PresentCapabilities {
        PresentCapabilities {
            mode: self.mode,
            partial_present: true,
            feedback: false,
        }
    }

    fn configure(&mut self, size: [u32; 2], _opaque: bool) -> Result<(), SkipReason> {
        if [size[0].max(1), size[1].max(1)] != self.size {
            self.allocate(size);
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

impl CpuPresenter for PixmapPresenter {
    fn extent(&self) -> [u32; 2] {
        self.size
    }

    fn image(&self, image: u32) -> Option<&Pixmap> {
        self.images.get(image as usize).map(|image| &image.pixels)
    }

    fn image_mut(&mut self, image: u32) -> Option<&mut Pixmap> {
        self.images
            .get_mut(image as usize)
            .map(|image| &mut image.pixels)
    }
}
