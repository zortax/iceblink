//! A pixmap presenter on a fake display that refreshes every period.

use std::any::Any;
use std::sync::Arc;

use iceblink::Presenter;
use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, PresentCapabilities, PresentFeedback, PresentResult, STALE_DAMAGE, SkipReason,
};
use iceblink_cpu::{CpuPresenter, Pixmap, PixmapPresenter};
use iceblink_driver::now_ns;

use crate::common::thread::probe::Probe;

/// A pixmap presenter on a display that refreshes at every multiple of `period_ns` on the frame
/// clock.
///
/// Its deadlines are the refreshes, unless it reports none. Its feedback reports the period and
/// a refresh from the first call on, and every present copies the image into the probe. A
/// presenter that shows its presents also reports each one, from the refresh it reached on.
pub(crate) struct Ticks {
    inner: PixmapPresenter,
    period_ns: u64,
    probe: Arc<Probe>,
    reported: bool,
    deadlines: bool,
    /// How long before a refresh a present must be made to reach it, if presents are reported.
    takes_ns: Option<u64>,
    /// The presents not reported yet, by number, and the refresh each reached.
    pending: Vec<(u64, u64)>,
    presents: u64,
}

impl Ticks {
    pub(crate) fn new(inner: PixmapPresenter, period_ns: u64, probe: Arc<Probe>) -> Self {
        Self {
            inner,
            period_ns,
            probe,
            reported: false,
            deadlines: true,
            takes_ns: None,
            pending: Vec::new(),
            presents: 0,
        }
    }

    /// Returns the presenter reporting each present at the first refresh at least `takes_ns`
    /// after it was made.
    pub(crate) fn showing_presents(mut self, takes_ns: u64) -> Self {
        self.takes_ns = Some(takes_ns);
        self
    }

    /// Notes a present made now, when presents are reported.
    pub(crate) fn note_present(&mut self) {
        let Some(takes) = self.takes_ns else {
            return;
        };
        self.presents += 1;
        let shown = (now_ns() + takes).div_ceil(self.period_ns) * self.period_ns;
        self.pending.push((self.presents, shown));
    }

    /// Returns the presenter without deadlines, so only its feedback tells the refreshes.
    pub(crate) fn without_deadlines(mut self) -> Self {
        self.deadlines = false;
        self
    }
}

impl Presenter for Ticks {
    fn capabilities(&self) -> PresentCapabilities {
        PresentCapabilities {
            feedback: true,
            ..self.inner.capabilities()
        }
    }

    fn configure(&mut self, size: [u32; 2], opaque: bool) -> Result<(), SkipReason> {
        self.inner.configure(size, opaque)
    }

    fn acquire(&mut self) -> Acquired {
        self.inner.acquire()
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
        let result = self.inner.present(image, damage);
        let shown = self.inner.image(image).cloned();
        self.probe.update(|state| {
            state.image = shown;
            state.presents += 1;
        });
        result
    }

    fn presents(&self) -> u64 {
        self.presents
    }

    fn next_deadline(&self) -> Option<u64> {
        self.deadlines
            .then(|| (now_ns() / self.period_ns + 1) * self.period_ns)
    }

    fn feedback(&mut self, out: &mut Vec<PresentFeedback>) {
        if !self.reported {
            self.reported = true;
            out.push(PresentFeedback {
                image: 0,
                present: 0,
                presented_ns: now_ns() / self.period_ns * self.period_ns,
                refresh_ns: self.period_ns,
            });
        }
        let now = now_ns();
        let period = self.period_ns;
        self.pending.retain(|&(present, shown)| {
            if shown > now {
                return true;
            }
            out.push(PresentFeedback {
                image: 0,
                present,
                presented_ns: shown,
                refresh_ns: period,
            });
            false
        });
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

impl CpuPresenter for Ticks {
    fn extent(&self) -> [u32; 2] {
        self.inner.extent()
    }

    fn image(&self, image: u32) -> Option<&Pixmap> {
        self.inner.image(image)
    }

    fn image_mut(&mut self, image: u32) -> Option<&mut Pixmap> {
        self.inner.image_mut(image)
    }

    fn is_configured(&self) -> bool {
        self.inner.is_configured()
    }
}
