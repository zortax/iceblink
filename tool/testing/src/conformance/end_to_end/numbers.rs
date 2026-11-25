//! Seeded pseudo-random numbers.

/// A generator of the same pseudo-random numbers for the same seed.
#[derive(Clone, Debug)]
pub(crate) struct Numbers(pub(crate) u64);

impl Numbers {
    /// Returns a number below `bound`.
    pub(crate) fn below(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }

    /// Returns a number in `[low, low + steps / 4)` on a quarter-pixel grid.
    pub(crate) fn quarter(&mut self, low: f32, steps: u64) -> f32 {
        low + self.below(steps) as f32 * 0.25
    }
}
