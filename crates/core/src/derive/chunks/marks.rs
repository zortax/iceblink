//! A quick test for chunk keys a set does not hold.

use iceblink::ChunkKey;

/// A 64-bit Bloom filter over chunk keys.
///
/// A key the filter does not report was never added. A key it reports may not have been.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Marks(u64);

impl Marks {
    pub(super) fn add(&mut self, key: ChunkKey) {
        self.0 |= bit(key);
    }

    /// Reports whether `key` may have been added.
    pub(super) fn may_hold(self, key: ChunkKey) -> bool {
        self.0 & bit(key) != 0
    }
}

/// Returns the bit that stands for `key`.
fn bit(key: ChunkKey) -> u64 {
    let mixed =
        (key.owner ^ u64::from(key.part).rotate_left(32)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    1 << (mixed >> 58)
}
