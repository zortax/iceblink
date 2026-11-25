//! When an output wants its next frame.

use crate::options::Pacing;
use crate::output::Output;

impl Output {
    /// Wants a frame for a client's call, which an idle paced output latches immediately and a
    /// stepped output builds at the next step.
    ///
    /// An output whose frames have not started wants none.
    pub(crate) fn ask(&mut self, now_ns: u64) {
        if !self.started {
            return;
        }
        // Feedback that came since the newest frame was presented can show it idle.
        if self.pacing == Pacing::Paced {
            self.read_feedback(now_ns);
        }
        let deadline = self.deadline();
        let budget = self.budget();
        self.schedule.ask(now_ns, deadline, budget);
    }

    /// Wants a frame for a client's call that starts the compositor's motion, which latches at
    /// the paced latch like the motion's later frames.
    ///
    /// A stepped output builds it at the next step, and an output whose frames have not started
    /// wants none.
    pub(crate) fn ask_paced(&mut self, now_ns: u64) {
        if self.pacing == Pacing::Stepped {
            self.ask(now_ns);
            return;
        }
        if !self.started {
            return;
        }
        self.read_feedback(now_ns);
        self.want(now_ns);
    }

    /// Wants the earliest frame that latches at or after `now_ns`, which a stepped output builds
    /// one interval after its previous frame.
    pub(super) fn want(&mut self, now_ns: u64) {
        let deadline = self.deadline();
        let budget = self.budget();
        self.schedule.want(now_ns, deadline, budget);
    }

    /// Wants the frame that shows the compositor's next change at `wake`, if any.
    pub(super) fn wake_at(&mut self, wake: Option<u64>, now_ns: u64) {
        if let Some(wake) = wake {
            let deadline = self.deadline();
            let budget = self.budget();
            self.schedule.wake(wake, now_ns, deadline, budget);
        }
    }

    /// Wants a frame the platform held again, soon enough to reach the display time
    /// `display_ns` it was built for while that is still ahead.
    pub(super) fn retry_held(&mut self, now_ns: u64, display_ns: u64) {
        let deadline = self.deadline();
        let budget = self.budget();
        self.schedule.retry(now_ns, display_ns, deadline, budget);
    }

    /// Returns the presenter's deadline for the next frame, which only a paced output follows.
    pub(super) fn deadline(&mut self) -> Option<u64> {
        if self.pacing == Pacing::Stepped {
            return None;
        }
        self.deadline = self
            .renderer
            .presenter()
            .and_then(|presenter| presenter.next_deadline());
        self.deadline
    }

    /// Returns the latch and display time of the frame due, when a commit queued at `now_ns` goes
    /// into it, as [`Schedule::reach`](crate::output::schedule::Schedule::reach) decides.
    pub(crate) fn reach(&self, now_ns: u64) -> Option<(u64, u64)> {
        let budget = self.budget();
        self.schedule.reach(now_ns, self.deadline, budget)
    }
}
