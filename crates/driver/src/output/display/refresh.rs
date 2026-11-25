//! The refresh interval a display measures from the times presents reached it.

/// The shortest and longest refresh interval a measurement accepts, 10 Hz to 500 Hz.
const SHORTEST_NS: u64 = 2_000_000;
const LONGEST_NS: u64 = 100_000_000;

/// The refresh interval, measured from the times presents reached the display.
///
/// The interval is the time between two presents divided by the number of refreshes between
/// them, over the latest run of presents that agree on it within 5 %.
#[derive(Debug, Default)]
pub(super) struct Refresh {
    /// The first present of the run the interval is measured over.
    first: Option<u64>,
    last: Option<u64>,
    pub(super) interval: Option<u64>,
}

impl Refresh {
    /// Takes the time a present reached the display into the interval measured so far.
    pub(super) fn observe(&mut self, presented_ns: u64) {
        let Some(last) = self.last.replace(presented_ns) else {
            self.first = Some(presented_ns);
            return;
        };
        let gap = presented_ns.saturating_sub(last);
        let plausible = (SHORTEST_NS..=LONGEST_NS).contains(&gap);
        let Some(interval) = self.interval else {
            if plausible {
                self.first = Some(last);
                self.interval = Some(gap);
            }
            return;
        };
        let first = self.first.unwrap_or(last);
        let span = presented_ns.saturating_sub(first);
        let refreshes = (span + interval / 2) / interval;
        if refreshes == 0 {
            return;
        }
        let measured = span / refreshes;
        if measured.abs_diff(interval) * 20 > interval {
            // The display changed its rate, so the run starts again.
            self.first = Some(last);
            self.interval = plausible.then_some(gap);
        } else {
            self.interval = Some(measured);
        }
    }
}
