//! Seeded pseudo-random numbers for randomized tests.

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
}
