//! What a schedule takes from the display times feedback reports.

use crate::options::Pacing;
use crate::output::schedule::Schedule;

impl Schedule {
    /// Moves the due frame of a paced output onto the refreshes followed now, as the refresh
    /// nearest the display time it was planned for, or the first one after `now_ns`.
    ///
    /// A latch that the move puts before `now_ns` is due immediately. A frame that follows
    /// presenter deadlines or latched immediately stays as it is.
    pub(crate) fn replan(&mut self, now_ns: u64, deadline: Option<u64>, budget_ns: u64) {
        if self.pacing != Pacing::Paced || self.due.is_none() || deadline.is_some() || self.at_once
        {
            return;
        }
        let wanted = self.target.saturating_sub(self.interval_ns / 2);
        let target = self.refresh_after(wanted.max(now_ns));
        self.due = Some(target.saturating_sub(self.lead(budget_ns)));
        self.target = target;
    }

    /// Moves the due frame of a paced output to an earlier refresh where the frames before it
    /// now allow one, at `now_ns`.
    ///
    /// The frame keeps the earliest display time it was wanted for, and its latch stays at or
    /// after `now_ns`. A frame that follows presenter deadlines or latched immediately stays as it
    /// is.
    pub(crate) fn pull_in(&mut self, now_ns: u64, deadline: Option<u64>, budget_ns: u64) {
        if self.pacing != Pacing::Paced || self.due.is_none() || deadline.is_some() || self.at_once
        {
            return;
        }
        let lead = self.lead(budget_ns);
        let target = self.refresh_after(self.earliest.max(now_ns + lead));
        if target < self.target {
            self.due = Some(target - lead);
            self.target = target;
        }
    }

    /// Notes that the frame built for display time `target` reached the display at `presented_ns`.
    ///
    /// When that frame is the newest and was shown at an earlier refresh than planned, its display
    /// time becomes the one it was shown at.
    ///
    /// Returns whether the newest frame's display time moved.
    pub(crate) fn shown(&mut self, target: u64, presented_ns: u64) -> bool {
        if self.shown != Some(target) || presented_ns + self.interval_ns / 2 >= target {
            return false;
        }
        self.shown = Some(presented_ns);
        self.early = None;
        if self.previous == Some(target) {
            self.previous = Some(presented_ns);
        }
        true
    }

    /// Takes `presented_ns`, when a present reached the display, as a measure of the display's
    /// refreshes, which later display times follow.
    pub(crate) fn anchor(&mut self, presented_ns: u64) {
        self.anchor.observe(presented_ns, self.interval_ns);
    }

    /// Reports whether `presented_ns`, when a present reached the display, was reported late, by
    /// more than a quarter of a millisecond and at most a quarter of an interval.
    pub(crate) fn reported_late(&self, presented_ns: u64) -> bool {
        self.anchor.reported_late(presented_ns, self.interval_ns)
    }
}
