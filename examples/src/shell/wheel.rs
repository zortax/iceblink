//! Wheel notches spread over a short ease.

/// How long the scroll of a wheel notch takes, in nanoseconds.
const DURATION_NS: f32 = 150_000_000.0;

/// The scroll distance of wheel notches that is still to be applied.
///
/// Each notch starts the ease again, over the distance it adds and the distance left.
#[derive(Default)]
pub(super) struct Wheel {
    ease: Option<Ease>,
}

#[derive(Clone, Copy)]
struct Ease {
    start_ns: u64,
    distance: [f32; 2],
    /// The part of `distance` already returned.
    applied: [f32; 2],
}

impl Wheel {
    /// Adds `delta` to the distance still to scroll, from `now_ns` on.
    pub(super) fn push(&mut self, delta: [f32; 2], now_ns: u64) {
        let left = self.ease.map_or([0.0; 2], |ease| {
            [
                ease.distance[0] - ease.applied[0],
                ease.distance[1] - ease.applied[1],
            ]
        });
        self.ease = Some(Ease {
            start_ns: now_ns,
            distance: [left[0] + delta[0], left[1] + delta[1]],
            applied: [0.0; 2],
        });
    }

    /// Returns the distance the ease covers by `at_ns` beyond what earlier calls returned, or
    /// `None` when no distance is left.
    pub(super) fn take(&mut self, at_ns: u64) -> Option<[f32; 2]> {
        let ease = self.ease.as_mut()?;
        let t = (at_ns.saturating_sub(ease.start_ns) as f32 / DURATION_NS).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        let reached = [ease.distance[0] * eased, ease.distance[1] * eased];
        let step = [reached[0] - ease.applied[0], reached[1] - ease.applied[1]];
        ease.applied = reached;
        if t >= 1.0 {
            self.ease = None;
        }
        Some(step)
    }

    /// Reports whether distance is left to scroll.
    pub(super) fn moving(&self) -> bool {
        self.ease.is_some()
    }
}
