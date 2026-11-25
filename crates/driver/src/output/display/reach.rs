//! How long before a refresh a present must be made to be shown at it.

/// The reach before any present was measured.
const DEFAULT_NS: u64 = 3_000_000;

/// How many frames made one reach ahead must reach their refresh before the reach is lowered.
const STEP_HITS: u32 = 8;

/// The time a present takes from being made to being shown at the earliest, which the display
/// server's latch and the device's work after the present add up to.
///
/// Presents prove two bounds of it:
///
/// - The upper bound is the shortest time a present took from being made to reaching the display.
/// - The lower bound is the longest time before a refresh a present was made and shown only at a
///   later one, when every earlier present had reached the display by then, the present was made
///   at a lowered reach, or another of the newest 16 frames made one reach ahead missed too.
///
/// The reach starts at 3 ms, or half the interval when that is shorter. Once a present was
/// measured, it stays there if that lies between the bounds, each widened by a sixteenth of an
/// interval, and otherwise starts at the widened upper bound. A frame made one reach ahead that
/// misses its refresh at the start moves the reach to the widened upper bound.
///
/// The reach is then lowered, as [Lowering](#lowering) describes. After that, a single frame that
/// misses its refresh changes nothing. When two of the newest 16 frames made one reach ahead
/// missed their refresh, the reach rises to a sixteenth of an interval beyond the time the
/// earliest of them was made before it.
///
/// An interval more than 5 % away from the one the bounds were measured at starts them again.
///
/// # Lowering
///
/// Each time 8 frames made one reach ahead reached their refresh, the reach is lowered by half its
/// distance to the lower bound, at least a thirty-second of an interval. A frame made one reach
/// ahead that misses its refresh after a lowering returns the reach to where it was before the
/// step. After the first such miss the reach is lowered by a thirty-second of an interval at a
/// time, and after the second it is not lowered again.
#[derive(Debug, Default)]
pub(super) struct Reach {
    /// The interval the bounds are measured at.
    interval: u64,
    /// The shortest time a present took from being made to reaching the display.
    upper: Option<u64>,
    /// The longest time before a refresh a present was made and shown only at a later one.
    lower: u64,
    /// The reach frames are made with, or `None` for the widened upper bound.
    level: Option<u64>,
    /// The reach before the latest step, if the reach was lowered.
    before: Option<u64>,
    /// Whether the reach was lowered since it last returned to where it was before a step.
    stepped: bool,
    /// How many times the reach returned to where it was before a step.
    overshot: u32,
    /// Whether the reach is no longer lowered.
    settled: bool,
    /// Frames that reached their refresh since the latest step.
    hits: u32,
    /// Whether each of the 16 newest judged frames missed its refresh, newest in the lowest bit.
    recent: u16,
    /// The longest time before its display time one of the newest missed frames was made.
    recent_missed: u64,
    /// The longest time before its display time a frame was made and repeatedly missed it.
    missed: Option<u64>,
}

impl Reach {
    /// Returns the reach at a refresh interval of `interval_ns`.
    pub(super) fn get(&self, interval_ns: u64) -> u64 {
        let margin = interval_ns / 16;
        let Some(upper) = self.upper else {
            return start(interval_ns);
        };
        let widened = upper + margin;
        let reach = self.level.map_or(widened, |level| level.min(widened));
        self.missed
            .map_or(reach, |missed| reach.max(missed + margin))
    }

    /// Returns how long before a refresh a present made later is certainly not shown at it.
    pub(super) fn lower(&self) -> u64 {
        self.lower
    }

    /// Takes a present made at `made_ns` that reached the display at `presented_ns`, at a
    /// refresh interval of `interval_ns`, into the bounds.
    ///
    /// `alone` reports whether every earlier present had reached the display by `made_ns`, so
    /// the present was shown at the first refresh it could reach.
    pub(super) fn shown(&mut self, made_ns: u64, presented_ns: u64, alone: bool, interval_ns: u64) {
        self.restart(interval_ns);
        let took = presented_ns.saturating_sub(made_ns);
        let first = self.upper.is_none();
        self.upper = Some(self.upper.map_or(took, |upper| upper.min(took)));
        if alone {
            self.lower = self.lower.max(took.saturating_sub(interval_ns));
        }
        let margin = interval_ns / 16;
        if first && (self.lower + margin..=took + margin).contains(&start(interval_ns)) {
            self.level = Some(start(interval_ns));
        }
    }

    /// Notes that a frame made one reach ahead reached its refresh, at a refresh interval of
    /// `interval_ns`.
    pub(super) fn hit(&mut self, interval_ns: u64) {
        self.recent <<= 1;
        if self.settled || self.upper.is_none() {
            return;
        }
        self.hits += 1;
        if self.hits < STEP_HITS {
            return;
        }
        self.hits = 0;
        let reach = self.get(interval_ns);
        let least = interval_ns / 32;
        let step = if self.overshot == 0 {
            (reach.saturating_sub(self.lower) / 2).max(least)
        } else {
            least
        };
        if reach < self.lower + least + step {
            self.settled = true;
            return;
        }
        self.before = Some(reach);
        self.level = Some(reach - step);
        self.stepped = true;
    }

    /// Notes that a frame made `ahead_ns` before its display time, at least one reach ahead,
    /// reached the display after it.
    pub(super) fn missed(&mut self, ahead_ns: u64) {
        if !self.settled {
            match self.before {
                Some(before) if self.stepped => {
                    self.lower = self.lower.max(ahead_ns);
                    self.stepped = false;
                    self.overshot += 1;
                    self.settled = self.overshot >= 2;
                    self.level = Some(before);
                    self.hits = 0;
                }
                // Frames made before the reach returned miss again and say nothing new.
                Some(_) => {}
                None => self.level = None,
            }
        }
        let repeated = self.recent != 0;
        self.recent = (self.recent << 1) | 1;
        self.recent_missed = if repeated {
            self.recent_missed.max(ahead_ns)
        } else {
            ahead_ns
        };
        if repeated {
            self.missed = Some(self.missed.unwrap_or(0).max(self.recent_missed));
            self.lower = self.lower.max(self.recent_missed);
            self.recent = 0;
        }
    }

    /// Starts the bounds again when `interval_ns` is more than 5 % away from the interval they
    /// were measured at.
    fn restart(&mut self, interval_ns: u64) {
        if self.interval.abs_diff(interval_ns) * 20 > self.interval {
            *self = Self {
                interval: interval_ns,
                ..Self::default()
            };
        }
    }
}

/// Returns the reach before any present was measured, at a refresh interval of `interval_ns`.
fn start(interval_ns: u64) -> u64 {
    DEFAULT_NS.min(interval_ns / 2)
}

#[cfg(test)]
mod tests {
    use super::Reach;

    const INTERVAL: u64 = 16_000_000;

    #[test]
    fn the_reach_is_lowered_until_a_frame_misses() {
        let mut reach = Reach::default();
        reach.shown(0, 25_000_000, true, INTERVAL);
        assert_eq!(
            reach.get(INTERVAL),
            26_000_000,
            "the lower bound lies beyond the start"
        );
        for _ in 0..8 {
            reach.hit(INTERVAL);
        }
        assert_eq!(reach.get(INTERVAL), 17_500_000);
        reach.missed(18_000_000);
        assert_eq!(reach.get(INTERVAL), 26_000_000, "a miss after a step");
        for _ in 0..8 {
            reach.hit(INTERVAL);
        }
        assert_eq!(reach.get(INTERVAL), 25_500_000, "then the steps are short");
        reach.missed(25_000_000);
        for _ in 0..8 {
            reach.hit(INTERVAL);
        }
        assert_eq!(
            reach.get(INTERVAL),
            26_000_000,
            "a second miss ends the lowering"
        );
    }

    #[test]
    fn one_miss_raises_nothing_and_two_close_ones_raise_the_reach() {
        let mut reach = Reach::default();
        reach.shown(0, 3_000_000, false, INTERVAL);
        for _ in 0..8 {
            reach.hit(INTERVAL);
        }
        reach.missed(2_000_000);
        for _ in 0..16 {
            reach.hit(INTERVAL);
        }
        let settled = (reach.get(INTERVAL), reach.lower());
        reach.missed(5_000_000);
        assert_eq!((reach.get(INTERVAL), reach.lower()), settled);
        reach.hit(INTERVAL);
        reach.missed(6_000_000);
        assert_eq!(reach.get(INTERVAL), 7_000_000);
    }
}
