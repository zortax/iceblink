//! A buffer-age presenter whose next acquisition can be made to fail.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iceblink::Presenter;
use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, BufferAge, PresentCapabilities, PresentFeedback, PresentMode, PresentResult,
    STALE_DAMAGE, SkipReason,
};

static NO_DEBT: DamageSet<STALE_DAMAGE> = DamageSet::new();

/// A buffer-age presenter of one image, whose next acquisition times out while `fail` holds.
pub(crate) struct Flaky {
    fail: Arc<AtomicBool>,
    acquired: Option<u32>,
    /// Whether the image holds the last frame.
    kept: bool,
}

impl Flaky {
    pub(crate) fn new(fail: Arc<AtomicBool>) -> Self {
        Self {
            fail,
            acquired: None,
            kept: false,
        }
    }
}

impl Presenter for Flaky {
    fn capabilities(&self) -> PresentCapabilities {
        PresentCapabilities {
            mode: PresentMode::BufferAge,
            partial_present: false,
            feedback: false,
        }
    }

    fn configure(&mut self, _size: [u32; 2], _opaque: bool) -> Result<(), SkipReason> {
        self.kept = false;
        Ok(())
    }

    fn acquire(&mut self) -> Acquired {
        if self.fail.swap(false, Ordering::Relaxed) {
            return Acquired::Skipped(SkipReason::Timeout);
        }
        self.acquired = Some(0);
        let age = if self.kept {
            BufferAge::Known(1)
        } else {
            BufferAge::Unknown
        };
        self.kept = true;
        Acquired::Image { image: 0, age }
    }

    fn acquired(&self) -> Option<u32> {
        self.acquired
    }

    fn release(&mut self, _image: u32) {
        self.acquired = None;
    }

    fn composed(&mut self, _damage: &DamageSet) {}

    fn debt(&self, _image: u32) -> &DamageSet<STALE_DAMAGE> {
        &NO_DEBT
    }

    fn present(&mut self, _image: u32, _damage: &DamageSet) -> PresentResult {
        self.acquired = None;
        PresentResult::Presented
    }

    fn next_deadline(&self) -> Option<u64> {
        None
    }

    fn feedback(&mut self, _out: &mut Vec<PresentFeedback>) {}
}
