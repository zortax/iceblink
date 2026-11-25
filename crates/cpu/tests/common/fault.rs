//! A pixmap presenter whose acquisitions and presents a test can make fail.

use std::any::Any;
use std::collections::VecDeque;

use iceblink::Presenter;
use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, PresentCapabilities, PresentFeedback, PresentResult, STALE_DAMAGE, SkipReason,
};
use iceblink_cpu::{CpuPresenter, Pixmap, PixmapPresenter};

/// A pixmap presenter that answers the next acquisitions and presents as a test queued.
#[derive(Debug)]
pub(crate) struct Faulty {
    pub(crate) inner: PixmapPresenter,
    /// Reasons the next acquisitions are skipped with, in order.
    pub(crate) acquisitions: VecDeque<SkipReason>,
    /// Results the next presents answer in place of presenting, in order.
    pub(crate) presents: VecDeque<PresentResult>,
    /// Whether every present answers `Held` once the queued results are used up.
    pub(crate) holding: bool,
    /// How many presents reached the inner presenter.
    pub(crate) presented: u32,
}

impl Faulty {
    /// Returns a faulty presenter around `inner` with nothing queued.
    pub(crate) fn new(inner: PixmapPresenter) -> Self {
        Self {
            inner,
            acquisitions: VecDeque::new(),
            presents: VecDeque::new(),
            holding: false,
            presented: 0,
        }
    }
}

impl Presenter for Faulty {
    fn capabilities(&self) -> PresentCapabilities {
        self.inner.capabilities()
    }

    fn configure(&mut self, size: [u32; 2], opaque: bool) -> Result<(), SkipReason> {
        self.inner.configure(size, opaque)
    }

    fn acquire(&mut self) -> Acquired {
        match self.acquisitions.pop_front() {
            Some(reason) => Acquired::Skipped(reason),
            None => self.inner.acquire(),
        }
    }

    fn acquired(&self) -> Option<u32> {
        self.inner.acquired()
    }

    fn release(&mut self, image: u32) {
        self.inner.release(image);
    }

    fn composed(&mut self, damage: &DamageSet) {
        self.inner.composed(damage);
    }

    fn debt(&self, image: u32) -> &DamageSet<STALE_DAMAGE> {
        self.inner.debt(image)
    }

    fn present(&mut self, image: u32, damage: &DamageSet) -> PresentResult {
        if let Some(result) = self.presents.pop_front() {
            return result;
        }
        if self.holding {
            return PresentResult::Held;
        }
        self.presented += 1;
        self.inner.present(image, damage)
    }

    fn next_deadline(&self) -> Option<u64> {
        None
    }

    fn feedback(&mut self, out: &mut Vec<PresentFeedback>) {
        self.inner.feedback(out);
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

impl CpuPresenter for Faulty {
    fn extent(&self) -> [u32; 2] {
        self.inner.extent()
    }

    fn image(&self, image: u32) -> Option<&Pixmap> {
        self.inner.image(image)
    }

    fn image_mut(&mut self, image: u32) -> Option<&mut Pixmap> {
        self.inner.image_mut(image)
    }
}
