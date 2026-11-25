//! The frames a schedule is asked for.

use crate::options::Pacing;
use crate::output::schedule::{FEEDBACK, Schedule};

/// The fraction of an interval a compositor's wake may fall after a refresh and still be shown at
/// that refresh.
const SNAP: u64 = 16;

/// How soon a paced output asks the platform again for a frame it held.
const RETRY_NS: u64 = 1_000_000;

impl Schedule {
    /// Wants a frame for a client's call at `now_ns`, which a stepped output builds at the next
    /// step.
    ///
    /// An idle paced output without a presenter `deadline` latches the frame immediately, for the
    /// display time it would be built for at its latch. Otherwise the frame is wanted as
    /// [`want`](Self::want) does.
    pub(crate) fn ask(&mut self, now_ns: u64, deadline: Option<u64>, budget_ns: u64) {
        match self.pacing {
            Pacing::Paced if deadline.is_none() && self.idle(now_ns) => {
                let target = self.refresh_after(now_ns + self.lead(budget_ns));
                self.offer(now_ns, target, target);
                self.at_once = true;
            }
            Pacing::Paced => self.want(now_ns, deadline, budget_ns),
            Pacing::Stepped => {
                let latch = self.previous.unwrap_or(0);
                self.offer(latch, latch, latch);
                self.asked = true;
            }
        }
    }

    /// Wants the earliest frame that latches at or after `now_ns`, unless one is due earlier.
    ///
    /// A stepped output's frame is due one interval after its previous frame.
    pub(crate) fn want(&mut self, now_ns: u64, deadline: Option<u64>, budget_ns: u64) {
        match self.pacing {
            Pacing::Paced => {
                let (latch, target) = self.wanted_frame(now_ns, deadline, budget_ns);
                self.offer(latch, target, now_ns + self.lead(budget_ns));
            }
            Pacing::Stepped => {
                let latch = self
                    .previous
                    .map_or(0, |previous| previous + self.interval_ns);
                self.offer(latch, latch, latch);
            }
        }
    }

    /// Returns the latch and display time of the frame due, when a client's call at `now_ns` goes
    /// into it, or `None` when no frame is due, when the frame due latches after the one the call
    /// would ask for, or on a stepped output.
    pub(crate) fn reach(
        &self,
        now_ns: u64,
        deadline: Option<u64>,
        budget_ns: u64,
    ) -> Option<(u64, u64)> {
        if self.pacing == Pacing::Stepped {
            return None;
        }
        let due = self.due?;
        let (asked, _) = self.wanted_frame(now_ns, deadline, budget_ns);
        (due <= asked).then_some((due, self.target))
    }

    /// Returns the latch and display time of the frame a paced output wants at `now_ns`.
    fn wanted_frame(&self, now_ns: u64, deadline: Option<u64>, budget_ns: u64) -> (u64, u64) {
        match deadline {
            Some(deadline) => (deadline, deadline + self.interval_ns),
            None => {
                let lead = self.lead(budget_ns);
                let target = self.refresh_after(now_ns + lead);
                (target - lead, target)
            }
        }
    }

    /// Wants the frame that reaches the display at `present_ns`, unless one is due earlier.
    ///
    /// A paced output without a presenter deadline counts a refresh up to a sixteenth of an
    /// interval before `present_ns` as reaching it. Such an output latches the frame just after
    /// the latest refresh from which a present made then still reaches the display at its display
    /// time, or one lead before the display time when that is earlier, so the frame of the
    /// compositor's own motion has the most time to reach the display.
    pub(crate) fn wake(
        &mut self,
        present_ns: u64,
        now_ns: u64,
        deadline: Option<u64>,
        budget_ns: u64,
    ) {
        match self.pacing {
            Pacing::Paced => {
                let (latch, target, earliest) = match deadline {
                    Some(deadline) => {
                        let target = present_ns.max(deadline + self.interval_ns);
                        (target - self.interval_ns, target, target)
                    }
                    None => {
                        // A wake a hair past a refresh is the compositor's own estimate of that
                        // refresh, off by the jitter of the measured display times.
                        let wanted = present_ns.saturating_sub(self.interval_ns / SNAP);
                        let lead = self.lead(budget_ns);
                        let target = self.refresh_after(wanted.max(now_ns + lead));
                        let after_refresh = target
                            .saturating_sub(self.refreshes_ahead() * self.interval_ns)
                            + self.interval_ns / FEEDBACK;
                        (after_refresh.min(target - lead), target, wanted)
                    }
                };
                self.offer(latch, target, earliest);
            }
            Pacing::Stepped => {
                let paced = self
                    .previous
                    .map_or(0, |previous| previous + self.interval_ns);
                let latch = present_ns.saturating_sub(self.interval_ns).max(paced);
                self.offer(latch, latch, latch);
            }
        }
    }

    /// Wants a frame the platform held again, a millisecond from `now_ns`, while more than two
    /// milliseconds remain before a present is certainly too late for the display time
    /// `display_ns` it was built for.
    ///
    /// Otherwise, and on a stepped output or one that follows presenter deadlines, the next frame
    /// is wanted as [`want`](Self::want) does.
    pub(crate) fn retry(
        &mut self,
        now_ns: u64,
        display_ns: u64,
        deadline: Option<u64>,
        budget_ns: u64,
    ) {
        let presented_by = display_ns.saturating_sub(self.too_late_ns);
        if self.pacing == Pacing::Paced
            && deadline.is_none()
            && now_ns + 2 * RETRY_NS < presented_by
        {
            self.offer(now_ns + RETRY_NS, display_ns, display_ns);
        } else {
            self.want(now_ns, deadline, budget_ns);
        }
    }
}
