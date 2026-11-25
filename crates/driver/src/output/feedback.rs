//! What an output takes from its presenter's feedback and the client's reports.

use crate::output::Output;

impl Output {
    /// Reads the presenter's feedback once the newest frame may have been shown at an earlier
    /// refresh than planned, by `now_ns`.
    pub(crate) fn check_early(&mut self, now_ns: u64) {
        if self.schedule.early().is_some_and(|early| early <= now_ns) {
            self.schedule.checked_early();
            self.read_feedback(now_ns);
        }
    }

    /// Takes the presenter's feedback and the client's reports into the schedule and the budget,
    /// at `now_ns`.
    ///
    /// The interval then comes from the first source that has one, as [`Display::interval`]
    /// picks it, and the depth from the presenter. When the refreshes moved, the interval changed
    /// or the depth changed, the due frame moves onto the new refreshes. When the newest frame was
    /// shown earlier than planned, the due frame moves to an earlier refresh.
    ///
    /// [`Display::interval`]: crate::output::display::Display::interval
    pub(super) fn read_feedback(&mut self, now_ns: u64) {
        let before = (self.schedule.grid(), self.schedule.depth);
        let presenter = self.renderer.presenter();
        let known = presenter
            .as_ref()
            .and_then(|presenter| presenter.refresh_ns())
            .or(self.refresh);
        self.schedule.depth = presenter
            .as_ref()
            .map_or(1, |presenter| u64::from(presenter.display_depth().max(1)));
        let earlier = presenter.is_some_and(|presenter| {
            self.display.read(
                presenter,
                known,
                &mut self.reported,
                &mut self.schedule,
                &mut self.budget,
            )
        });

        self.schedule.interval_ns = self.display.interval(known);
        let changed = !same_grid(before.0, self.schedule.grid()) || before.1 != self.schedule.depth;
        if changed || earlier {
            let deadline = self.deadline();
            let budget = self.budget.get(self.schedule.interval_ns);
            if changed {
                self.schedule.replan(now_ns, deadline, budget);
            }
            if earlier {
                self.schedule.pull_in(now_ns, deadline, budget);
            }
        }
    }
}

/// Reports whether two sets of refreshes, each one refresh and the interval, are the same within
/// a quarter of a millisecond.
fn same_grid(a: (Option<u64>, u64), b: (Option<u64>, u64)) -> bool {
    const TOLERANCE_NS: u64 = 250_000;
    match (a, b) {
        ((None, _), (None, _)) => true,
        ((Some(a), interval), (Some(b), other)) if interval == other && interval > 0 => {
            let phase = a.abs_diff(b) % interval;
            phase.min(interval - phase) <= TOLERANCE_NS
        }
        _ => false,
    }
}
