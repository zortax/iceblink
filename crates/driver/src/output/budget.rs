//! The time a paced output reserves between latching a frame and the frame's display time.

/// How many recent frames the measured budget looks back over.
const FRAMES: usize = 16;

/// The smallest measured budget at a refresh interval of 8 ms or longer, which covers how late a
/// driver thread wakes for its latch and how early a display server takes a frame.
const FLOOR_NS: u64 = 4_000_000;

/// How much a missed refresh grows a measured budget.
const MISS_NS: u64 = 1_000_000;

/// The fewest intervals between two growths of the margin for missed refreshes.
const MISS_SPACING: u64 = 4;

/// An output's latch budget: a fixed one, or one measured from its recent frames.
///
/// A measured budget is the second slowest recent frame's cost plus a quarter, at least the floor,
/// plus a margin for missed refreshes, and at most one interval. With one frame measured, that
/// frame's cost counts, and before any the budget is one interval. The floor is 4 ms, or half the
/// interval when that is shorter. The margin raises the budget to at most three quarters of the
/// interval. A fixed budget is at most one interval.
///
/// The first frame after an output is made or configured counts at most the floor, since it also
/// pays for setting up its target. A frame's cost is the time from its latch to the end of its
/// draw, without the time the draw waited for an image.
#[derive(Debug)]
pub(crate) struct Budget {
    fixed: Option<u64>,
    costs: [u64; FRAMES],
    next: usize,
    measured: usize,
    /// Time added for refreshes the frames missed.
    margin: u64,
    /// Frames that reached their refresh since the margin last shrank.
    hits: u32,
    /// When the margin last grew, as the display time of the frame that missed.
    grown: Option<u64>,
}

impl Budget {
    pub(crate) fn new(fixed: Option<u64>) -> Self {
        Self {
            fixed,
            costs: [0; FRAMES],
            next: 0,
            measured: 0,
            margin: 0,
            hits: 0,
            grown: None,
        }
    }

    /// Returns the budget at a refresh interval of `interval_ns`.
    pub(crate) fn get(&self, interval_ns: u64) -> u64 {
        if let Some(fixed) = self.fixed {
            return fixed.min(interval_ns);
        }
        if self.measured == 0 {
            return interval_ns;
        }
        // One expensive frame, such as the first after a launch, does not set the budget.
        let mut costs = self.costs;
        let recent = &mut costs[..self.measured];
        recent.sort_unstable();
        let cost = recent[self.measured.saturating_sub(2)];
        let base = (cost + cost / 4).max(floor(interval_ns));
        // A call made just after a refresh still reaches the next latch.
        let margin = self.margin.min((interval_ns * 3 / 4).saturating_sub(base));
        (base + margin).min(interval_ns)
    }

    /// Notes that a frame reached the display at `shown_ns`, after the refresh it was built for,
    /// which grows a measured budget by 1 ms, at most once every 4 intervals of `interval_ns`.
    pub(crate) fn missed(&mut self, shown_ns: u64, interval_ns: u64) {
        self.hits = 0;
        if self
            .grown
            .is_some_and(|grown| shown_ns < grown + MISS_SPACING * interval_ns)
        {
            return;
        }
        self.grown = Some(shown_ns);
        // A margin beyond the most the budget takes would only delay its shrinking.
        self.margin = (self.margin + MISS_NS).min(interval_ns * 3 / 4);
    }

    /// Notes that a frame reached the refresh it was built for, so a margin unused for 120 frames
    /// shrinks by a quarter of a millisecond.
    pub(crate) fn hit(&mut self) {
        self.hits += 1;
        if self.hits >= 120 {
            self.hits = 0;
            self.margin = self.margin.saturating_sub(MISS_NS / 4);
        }
    }

    /// Notes the cost of a frame that also set up its target, of which at most the floor at a
    /// refresh interval of `interval_ns` counts.
    pub(crate) fn measure_setup(&mut self, cost_ns: u64, interval_ns: u64) {
        self.measure(cost_ns.min(floor(interval_ns)));
    }

    /// Notes the cost of a frame.
    pub(crate) fn measure(&mut self, cost_ns: u64) {
        self.costs[self.next] = cost_ns;
        self.next = (self.next + 1) % FRAMES;
        self.measured = (self.measured + 1).min(FRAMES);
    }
}

/// Returns the smallest measured budget at a refresh interval of `interval_ns`.
fn floor(interval_ns: u64) -> u64 {
    FLOOR_NS.min(interval_ns / 2)
}

#[cfg(test)]
mod tests {
    use super::{Budget, FLOOR_NS, MISS_NS};

    const INTERVAL: u64 = 16_000_000;

    #[test]
    fn misses_in_a_burst_grow_the_budget_once() {
        let mut budget = Budget::new(None);
        budget.measure(1_000_000);
        for refresh in 1..4 {
            budget.missed(refresh * INTERVAL, INTERVAL);
        }
        assert_eq!(budget.get(INTERVAL), FLOOR_NS + MISS_NS);
        budget.missed(5 * INTERVAL, INTERVAL);
        assert_eq!(budget.get(INTERVAL), FLOOR_NS + 2 * MISS_NS);
    }
}
