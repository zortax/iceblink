//! When an output's next frame latches, and the clock it is built with.

mod feedback;
mod start;
mod want;

use crate::options::Pacing;
use crate::output::anchor::Anchor;

/// The fraction of an interval after a refresh by which the display's feedback for it arrived.
const FEEDBACK: u64 = 16;

/// An output's frame schedule.
///
/// A paced output's frame is built for a display time. Its lead is the budget plus one interval
/// for each present the display holds besides the frame's, since a present made just after a
/// refresh reaches the display that many refreshes later. When the frame latches depends on what
/// wants it:
///
/// - If the presenter has deadlines, the frame latches at the deadline.
/// - If a client's call wants the frame on an idle output, it latches immediately.
/// - If the compositor's own motion wants the frame, it latches just after the refresh from which
///   a present reaches the display at its display time, or one lead before the display time when
///   that is earlier.
/// - Otherwise it latches one lead before its display time.
///
/// An output is idle when no frame is due and the refresh after the newest frame it presented has
/// passed.
///
/// A stepped output's frame is due at the next step when a client's call wants it, and otherwise
/// no sooner than one interval after the previous frame was built.
#[derive(Debug)]
pub(crate) struct Schedule {
    pacing: Pacing,
    /// When the next frame latches, if one is wanted.
    due: Option<u64>,
    /// Whether a client's call wants a stepped output's next frame at the next step.
    asked: bool,
    /// Whether the due frame latched immediately, for a client's call on an idle paced output.
    at_once: bool,
    /// The display time the due frame is built for.
    target: u64,
    /// The earliest display time the due frame may be moved to.
    earliest: u64,
    /// The display time of the previous frame. Without an anchor it is the earliest time the
    /// frame can reach the display, and on a stepped output when the frame was built.
    previous: Option<u64>,
    /// The display time of the newest frame a paced output presented.
    shown: Option<u64>,
    /// The previous and the newest frame's display times before the latest paced frame started.
    before: (Option<u64>, Option<u64>),
    /// When feedback may first show the newest frame at an earlier refresh than planned.
    early: Option<u64>,
    /// The display's refreshes, which display times follow.
    anchor: Anchor,
    pub(crate) interval_ns: u64,
    /// How many presents the display holds at once, at least one.
    pub(crate) depth: u64,
}

impl Schedule {
    pub(crate) fn new(pacing: Pacing, interval_ns: u64) -> Self {
        Self {
            pacing,
            due: None,
            asked: false,
            at_once: false,
            target: 0,
            earliest: 0,
            previous: None,
            shown: None,
            before: (None, None),
            early: None,
            anchor: Anchor::default(),
            interval_ns,
            depth: 1,
        }
    }

    pub(crate) fn due(&self) -> Option<u64> {
        self.due
    }

    /// Reports whether the due frame latched immediately, for a client's call on an idle output.
    pub(crate) fn at_once(&self) -> bool {
        self.at_once
    }

    /// Reports whether a frame is due at a step at `now_ns`.
    ///
    /// A frame a client's call wants is due at any step.
    pub(crate) fn due_by(&self, now_ns: u64) -> bool {
        self.asked || self.due.is_some_and(|due| due <= now_ns)
    }

    /// Returns when the due frame is predicted to reach the display, if a frame is due.
    ///
    /// A stepped frame is predicted one interval after it is due.
    pub(crate) fn display(&self) -> Option<u64> {
        let due = self.due?;
        Some(match self.pacing {
            Pacing::Paced => self.target,
            Pacing::Stepped => due + self.interval_ns,
        })
    }

    /// Returns when feedback may first show the newest frame at an earlier refresh than planned,
    /// if it can be.
    pub(crate) fn early(&self) -> Option<u64> {
        self.early
    }

    /// Forgets when the newest frame may be shown early, once its feedback was read.
    pub(crate) fn checked_early(&mut self) {
        self.early = None;
    }

    /// Returns the refreshes display times follow: one of them and the interval, if there are
    /// any.
    pub(crate) fn grid(&self) -> (Option<u64>, u64) {
        (self.anchor.refresh(), self.interval_ns)
    }

    /// Stops wanting a frame until something asks for one.
    pub(crate) fn park(&mut self) {
        self.due = None;
        self.asked = false;
        self.at_once = false;
    }

    /// Reports whether display times follow the refreshes of the display.
    pub(crate) fn anchored(&self) -> bool {
        self.anchor.refresh().is_some()
    }

    /// Reports whether no frame is due and the refresh after the newest frame presented has
    /// passed by `now_ns`.
    fn idle(&self, now_ns: u64) -> bool {
        self.due.is_none()
            && self
                .shown
                .is_none_or(|shown| shown + self.interval_ns <= now_ns)
    }

    /// Wants the frame that latches at `latch` for display time `target`, unless one is due
    /// earlier, and lets it move to no earlier display time than `earliest`.
    fn offer(&mut self, latch: u64, target: u64, earliest: u64) {
        if self.due.is_none_or(|due| latch < due) {
            self.due = Some(latch);
            self.target = target;
            self.earliest = earliest;
        }
    }

    /// Returns the lead of a paced frame with a budget of `budget_ns`.
    fn lead(&self, budget_ns: u64) -> u64 {
        budget_ns + self.queued()
    }

    /// Returns how long a present waits behind those the display holds: one interval for each
    /// present it holds besides the frame's.
    fn queued(&self) -> u64 {
        self.depth.saturating_sub(1) * self.interval_ns
    }

    /// Returns the earliest display time at or after `at` that follows the previous frame's.
    ///
    /// With an anchor the display times are its refreshes, at least half an interval after the
    /// previous frame's. Without one they are at least one interval apart.
    fn refresh_after(&self, at: u64) -> u64 {
        match self.anchor.refresh() {
            Some(anchor) => {
                let floor = self
                    .previous
                    .map_or(at, |previous| at.max(previous + self.interval_ns / 2));
                if floor <= anchor {
                    return anchor;
                }
                let periods = (floor - anchor).div_ceil(self.interval_ns);
                anchor + periods * self.interval_ns
            }
            None => self
                .previous
                .map_or(at, |previous| at.max(previous + self.interval_ns)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Schedule;
    use crate::options::Pacing;

    const INTERVAL: u64 = 16_000_000;

    #[test]
    fn a_wake_just_past_a_refresh_is_shown_at_it() {
        let mut schedule = Schedule::new(Pacing::Paced, INTERVAL);
        schedule.anchor(0);
        schedule.wake(2 * INTERVAL + 50_000, 0, None, 4_000_000);
        assert_eq!(schedule.display(), Some(2 * INTERVAL));
    }
}
