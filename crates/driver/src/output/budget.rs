//! The time a paced output reserves between latching a frame and presenting it.

/// How many recent frames the measured budget looks back over.
const FRAMES: usize = 16;

/// The smallest measured budget at a refresh interval of 2 ms or longer, which covers how late a
/// driver thread wakes for its latch.
const FLOOR_NS: u64 = 1_000_000;

/// An output's latch budget: a fixed one, or one measured from its recent frames.
///
/// A measured budget is the second slowest recent frame's cost plus a quarter, at least the floor
/// and at most one interval. With one frame measured, that frame's cost counts. Before any, the
/// budget is what remains of one interval after the reach, at least the floor. The floor is 1 ms,
/// or half the interval when that is shorter. A fixed budget is at most one interval.
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
}

impl Budget {
    pub(crate) fn new(fixed: Option<u64>) -> Self {
        Self {
            fixed,
            costs: [0; FRAMES],
            next: 0,
            measured: 0,
        }
    }

    /// Returns the budget at a refresh interval of `interval_ns` and a reach of `reach_ns`.
    pub(crate) fn get(&self, interval_ns: u64, reach_ns: u64) -> u64 {
        if let Some(fixed) = self.fixed {
            return fixed.min(interval_ns);
        }
        if self.measured == 0 {
            return interval_ns.saturating_sub(reach_ns).max(floor(interval_ns));
        }
        // One expensive frame, such as the first after a launch, does not set the budget.
        let mut costs = self.costs;
        let recent = &mut costs[..self.measured];
        recent.sort_unstable();
        let cost = recent[self.measured.saturating_sub(2)];
        (cost + cost / 4).max(floor(interval_ns)).min(interval_ns)
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
