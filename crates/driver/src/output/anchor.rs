//! The refreshes of a display, from the times presents reached it.

/// How much later than the refreshes followed so far a display time may be and still move them.
const LATE_NS: u64 = 250_000;

/// How many display times later than that, in a row, move the refreshes to the earliest of them.
const LATE_RUN: u32 = 8;

/// How many intervals away from the refreshes followed so far a display time replaces them.
const STALE: u64 = 64;

/// The refreshes of a display, as one refresh that later refreshes follow every interval.
///
/// A display time is never earlier than the refresh it reports. The refreshes follow a
/// display time that is at most a quarter of a millisecond later than them, or earlier. Later
/// display times move them only when eight come in a row, which a display that changed its phase
/// reports, and then to the earliest of those. A display time 64 intervals or more away from the
/// refresh followed so far replaces it.
#[derive(Debug, Default)]
pub(crate) struct Anchor {
    refresh: Option<u64>,
    /// The earliest of the late display times in a row, how late it was, and how many came.
    late: Option<(u64, u64, u32)>,
}

impl Anchor {
    pub(crate) fn refresh(&self) -> Option<u64> {
        self.refresh
    }

    /// Reports whether `presented_ns`, when a present reached the display, is later than the
    /// refreshes followed so far, which refresh every `interval_ns`, by more than a quarter of a
    /// millisecond and at most a quarter of an interval.
    ///
    /// A display time further off says that the interval or the phase changed.
    pub(crate) fn reported_late(&self, presented_ns: u64, interval_ns: u64) -> bool {
        self.refresh.is_some_and(|refresh| {
            (LATE_NS + 1..=interval_ns / 4).contains(&late_by(refresh, presented_ns, interval_ns))
        })
    }

    /// Takes `presented_ns`, when a present reached the display, which refreshes every
    /// `interval_ns`.
    pub(crate) fn observe(&mut self, presented_ns: u64, interval_ns: u64) {
        let Some(refresh) = self.refresh else {
            self.refresh = Some(presented_ns);
            return;
        };
        let late_ns = late_by(refresh, presented_ns, interval_ns);
        if late_ns <= LATE_NS || presented_ns.abs_diff(refresh) >= STALE * interval_ns {
            self.refresh = Some(presented_ns);
            self.late = None;
            return;
        }
        let (earliest, by, count) = match self.late {
            Some((earliest, by, count)) if by <= late_ns => (earliest, by, count + 1),
            Some((_, _, count)) => (presented_ns, late_ns, count + 1),
            None => (presented_ns, late_ns, 1),
        };
        if count >= LATE_RUN {
            self.refresh = Some(earliest);
            self.late = None;
        } else {
            self.late = Some((earliest, by, count));
        }
    }
}

/// Returns how much later `at` is than the nearest refresh after `refresh`, or zero when it is
/// nearer to the refresh after it.
fn late_by(refresh: u64, at: u64, interval_ns: u64) -> u64 {
    let phase = if at >= refresh {
        (at - refresh) % interval_ns
    } else {
        (interval_ns - (refresh - at) % interval_ns) % interval_ns
    };
    if phase < interval_ns / 2 { phase } else { 0 }
}
