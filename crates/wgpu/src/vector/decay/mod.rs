//! When a scratch texture grows and when it shrinks again.

mod extent;

use web_time::{Duration, Instant};

pub use self::extent::Extent;

/// The allocation policy of one scratch texture.
///
/// Growth is immediate. A shrink waits until demand has stayed below the allocation for
/// [`GRACE`](Self::GRACE), and then takes the largest extent asked for during that wait.
// A scroll or a resize is a run of frames whose demand swings, and reallocating in the middle of
// one costs more than the memory is worth.
#[derive(Clone, Copy, Debug, Default)]
pub struct Decay {
    /// The extent allocated now.
    held: Extent,
    /// The largest extent asked for since the wait began.
    recent: Extent,
    /// When the wait began.
    quiet_since: Option<Instant>,
}

impl Decay {
    /// How long demand stays below the allocation before the allocation shrinks.
    pub const GRACE: Duration = Duration::from_secs(2);

    /// Returns the policy of a scratch with nothing allocated.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the extent allocated now.
    pub fn held(&self) -> Extent {
        self.held
    }

    /// Forgets the allocation.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Returns the extent to reallocate to for a frame that needs `want`, or `None` to keep the
    /// allocation.
    ///
    /// The clock reading is [`Instant::now`].
    pub fn wants(&mut self, want: Extent) -> Option<Extent> {
        self.wants_at(want, Instant::now())
    }

    /// Returns the extent to reallocate to for a frame at `now` that needs `want`, or `None` to
    /// keep the allocation.
    ///
    /// The answer depends on the allocation:
    ///
    /// - If nothing is allocated, or the allocation does not cover `want`, the result is the
    ///   union of the two, and the wait starts again.
    /// - If the frames since the wait began asked for the whole allocation, the result is `None`,
    ///   and the wait starts again.
    /// - If the wait has lasted [`GRACE`](Self::GRACE), the result is the largest extent asked for
    ///   during it, or `None` when that is the allocation, and the wait starts again.
    /// - Otherwise the result is `None`.
    pub fn wants_at(&mut self, want: Extent, now: Instant) -> Option<Extent> {
        if !self.held.covers(want) || self.held == Extent::NONE {
            self.held = self.held.union(want);
            // The growing frame is why the texture has this size, so the wait starts after it.
            // Counting it would make the first wait find its maximum equal to the allocation.
            self.recent = Extent::NONE;
            self.quiet_since = Some(now);
            return Some(self.held);
        }

        self.recent = self.recent.union(want);
        if self.recent == self.held {
            self.recent = Extent::NONE;
            self.quiet_since = Some(now);
            return None;
        }

        let since = self.quiet_since.get_or_insert(now);
        if now.saturating_duration_since(*since) < Self::GRACE {
            return None;
        }

        let settled = self.recent;
        self.recent = Extent::NONE;
        self.quiet_since = Some(now);
        if settled == self.held {
            return None;
        }
        self.held = settled;
        Some(settled)
    }
}
