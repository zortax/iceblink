//! The refreshes frames are planned on, and whether the display times agree with them.

/// How far from a refresh a display time may lie and still agree with the refreshes.
const AGREE_NS: u64 = 250_000;

/// The refreshes frames are planned on, as the renderer last reported them.
///
/// A wait may sleep until a refresh only while the refreshes agree with the display. They agree
/// when the newest present was seen to reach the display within a quarter of a millisecond of a
/// refresh, while the waiter waited for it.
#[derive(Debug, Default)]
pub(super) struct Refreshes {
    /// One refresh and the interval, on the frame clock.
    grid: Option<(u64, u64)>,
    /// Whether the display times agree with the refreshes.
    agreed: bool,
}

impl Refreshes {
    /// Takes the refreshes at `refresh_ns` and every `interval_ns` from it, or none for a zero
    /// interval.
    pub(super) fn follow(&mut self, refresh_ns: u64, interval_ns: u64) {
        self.grid = (interval_ns > 0).then_some((refresh_ns, interval_ns));
    }

    /// Returns the refresh interval, if known.
    pub(super) fn interval(&self) -> Option<u64> {
        self.grid.map(|(_, interval)| interval)
    }

    /// Returns the first refresh at or after `at_ns` and the interval, while the display times
    /// agree with the refreshes.
    pub(super) fn first_after(&self, at_ns: u64) -> Option<(u64, u64)> {
        let (refresh, interval) = self.grid.filter(|_| self.agreed)?;
        Some((at_or_after(refresh, interval, at_ns), interval))
    }

    /// Takes `shown_ns`, when a present was seen to reach the display, as agreeing with the
    /// refreshes or not.
    ///
    /// A present found shown when a wait began, after the waiter slept, was shown at a time the
    /// waiter did not see, and so the refreshes do not agree.
    pub(super) fn observe(&mut self, shown_ns: u64, seen: bool) {
        self.agreed = seen
            && self.grid.is_some_and(|(refresh, interval)| {
                let after = at_or_after(refresh, interval, shown_ns);
                let before = after.saturating_sub(interval);
                (after - shown_ns).min(shown_ns - before) <= AGREE_NS
            });
    }
}

/// Returns the first time at or after `at` of the refreshes at `refresh` and every `interval`
/// from it.
fn at_or_after(refresh: u64, interval: u64, at: u64) -> u64 {
    if at <= refresh {
        refresh - (refresh - at) / interval * interval
    } else {
        refresh + (at - refresh).div_ceil(interval) * interval
    }
}

#[cfg(test)]
mod tests {
    use super::at_or_after;

    #[test]
    fn finds_the_first_refresh_at_or_after_a_time() {
        assert_eq!(at_or_after(100, 10, 100), 100);
        assert_eq!(at_or_after(100, 10, 101), 110);
        assert_eq!(at_or_after(100, 10, 75), 80);
        assert_eq!(at_or_after(100, 10, 70), 70);
    }
}
