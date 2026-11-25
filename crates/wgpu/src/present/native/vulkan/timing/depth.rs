//! How many presents may wait for the display at once.

use std::collections::VecDeque;

use crate::present::FRAME_LATENCY;

/// How many of the newest shown presents the depth is measured over.
const MEASURED: usize = 16;

/// How long a present held behind the one before it takes to be submitted once that one is shown.
// The waiter notices the shown present and the driver retries every millisecond, which took up to
// about 1.5 ms in fakeland at 120 Hz.
const RELEASE_NS: u64 = 2_000_000;

/// The display's pipeline depth: how many presents it holds at once when it is given one every
/// refresh.
///
/// The depth is the shortest time a recent present took from its submission to the display, plus
/// [`RELEASE_NS`], in refreshes, rounded up. It is one until a present was shown and the refresh
/// interval is known, and at most [`FRAME_LATENCY`].
#[derive(Debug, Default)]
pub(super) struct Depth {
    /// How long each of the newest shown presents took to reach the display, oldest first.
    latencies: VecDeque<u64>,
}

impl Depth {
    /// Takes a present submitted at `submitted_ns` and shown at `shown_ns` into the measurement.
    pub(super) fn observe(&mut self, submitted_ns: u64, shown_ns: u64) {
        if self.latencies.len() == MEASURED {
            self.latencies.pop_front();
        }
        self.latencies
            .push_back(shown_ns.saturating_sub(submitted_ns));
    }

    /// Returns the depth on a display that refreshes every `interval_ns`, if known.
    pub(super) fn get(&self, interval_ns: Option<u64>) -> u64 {
        let interval = interval_ns.filter(|interval| *interval > 0);
        let shortest = self.latencies.iter().min();
        let (Some(interval), Some(shortest)) = (interval, shortest) else {
            return 1;
        };
        // A present held until the one before it is shown goes out up to a release later, too late
        // for a display that latches the next refresh's presents sooner than that.
        shortest
            .saturating_add(RELEASE_NS)
            .div_ceil(interval)
            .clamp(1, u64::from(FRAME_LATENCY))
    }
}

#[cfg(test)]
mod tests {
    use super::Depth;

    const INTERVAL: u64 = 8_333_333;

    #[test]
    fn is_one_until_a_present_was_shown_on_a_known_refresh() {
        let mut depth = Depth::default();
        assert_eq!(depth.get(Some(INTERVAL)), 1);
        depth.observe(0, 2 * INTERVAL);
        assert_eq!(depth.get(None), 1);
    }

    #[test]
    fn counts_the_refreshes_the_quickest_present_took() {
        let mut depth = Depth::default();
        depth.observe(0, 4_000_000);
        assert_eq!(depth.get(Some(INTERVAL)), 1);

        let mut depth = Depth::default();
        depth.observe(0, 2 * INTERVAL - 3_000_000);
        depth.observe(INTERVAL, 3 * INTERVAL);
        assert_eq!(depth.get(Some(INTERVAL)), 2);
    }

    #[test]
    fn a_present_queued_behind_others_does_not_deepen_it() {
        let mut depth = Depth::default();
        depth.observe(0, 4_000_000);
        depth.observe(INTERVAL, 3 * INTERVAL);
        assert_eq!(depth.get(Some(INTERVAL)), 1);
    }

    #[test]
    fn a_present_shown_within_a_release_of_a_refresh_counts_one_deeper() {
        let mut depth = Depth::default();
        depth.observe(0, 8_100_000);
        assert_eq!(depth.get(Some(INTERVAL)), 2);
    }
}
