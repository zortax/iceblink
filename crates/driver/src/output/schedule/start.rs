//! The start of a scheduled frame.

use iceblink::FrameClock;

use crate::options::Pacing;
use crate::output::schedule::{FEEDBACK, Schedule};

impl Schedule {
    /// Starts the due frame of a paced output at `now_ns` and returns its clock.
    ///
    /// Without an anchor the frame may reach the display as soon as it is drawn and the presents
    /// the display holds before it are shown, so the frames after it are spaced from then.
    pub(crate) fn start(&mut self, now_ns: u64, budget_ns: u64) -> FrameClock {
        let present_ns = if now_ns + self.queued() < self.target {
            self.target
        } else {
            self.refresh_after(now_ns + self.lead(budget_ns))
        };
        self.early = (self.pacing == Pacing::Paced && self.anchored())
            .then(|| self.refresh_after(now_ns + self.queued()))
            .filter(|&earliest| earliest < present_ns)
            .map(|earliest| earliest + self.interval_ns / FEEDBACK);
        self.park();
        self.before = (self.previous, self.shown);
        self.previous = Some(if self.anchored() {
            present_ns
        } else {
            now_ns + self.queued()
        });
        self.shown = Some(present_ns);
        FrameClock {
            now_ns,
            present_ns,
            interval_ns: self.interval_ns,
        }
    }

    /// Forgets the paced frame started last, which showed nothing, so later frames follow the
    /// frame before it.
    pub(crate) fn showed_nothing(&mut self) {
        (self.previous, self.shown) = self.before;
        self.early = None;
    }

    /// Starts the due frame of a stepped output at `clock`.
    pub(crate) fn step(&mut self, clock: &FrameClock) {
        self.park();
        self.previous = Some(clock.now_ns);
        if clock.interval_ns > 0 {
            self.interval_ns = clock.interval_ns;
        }
    }
}
