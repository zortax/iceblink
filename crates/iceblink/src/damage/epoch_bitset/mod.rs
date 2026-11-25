//! A visited set that is emptied in constant time.

/// The stamp of an index never visited in any epoch.
const NEVER: u32 = 0;

/// A set of visited indices that is emptied in constant time.
///
/// [`EpochBitset::bump`] empties the set without touching the stored indices. When the epoch
/// counter runs out, about once every 2³² bumps, the bump clears the stored indices.
///
/// ```
/// use iceblink::damage::EpochBitset;
///
/// let mut visited = EpochBitset::new();
/// assert!(visited.visit(7));        // first time this epoch
/// assert!(!visited.visit(7));       // already seen
/// assert!(visited.contains(7));
///
/// visited.bump();                   // a new walk begins
/// assert!(!visited.contains(7));
/// assert!(visited.visit(7));
/// ```
#[derive(Clone, Debug)]
pub struct EpochBitset {
    /// The epoch each index was last visited in, or [`NEVER`].
    stamps: Vec<u32>,
    /// The epoch being marked now, which is never [`NEVER`].
    epoch: u32,
}

impl EpochBitset {
    /// Creates an empty set in its first epoch.
    pub const fn new() -> Self {
        Self {
            stamps: Vec::new(),
            epoch: NEVER + 1,
        }
    }

    /// Creates an empty set with room for indices below `capacity` before it reallocates.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            stamps: Vec::with_capacity(capacity),
            epoch: NEVER + 1,
        }
    }

    /// Returns the epoch currently being marked.
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Ends the current epoch and begins the next, forgetting every visit.
    pub fn bump(&mut self) {
        // Each index keeps the epoch it was last visited in, so a new epoch forgets every visit.
        // The bump that would wrap the counter clears the stamps instead.
        match self.epoch.checked_add(1) {
            Some(next) => self.epoch = next,
            None => {
                self.stamps.fill(NEVER);
                self.epoch = NEVER + 1;
            }
        }
    }

    /// Marks `index` visited and returns whether this is its first visit in the current epoch.
    ///
    /// The set grows to fit `index`.
    pub fn visit(&mut self, index: usize) -> bool {
        if index >= self.stamps.len() {
            self.stamps.resize(index + 1, NEVER);
        }
        let stamp = &mut self.stamps[index];
        let first = *stamp != self.epoch;
        *stamp = self.epoch;
        first
    }

    /// Reports whether `index` has been visited in the current epoch, without marking it.
    pub fn contains(&self, index: usize) -> bool {
        self.stamps.get(index).copied() == Some(self.epoch)
    }

    /// Forgets `index`'s visit in the current epoch.
    pub fn forget(&mut self, index: usize) {
        if let Some(stamp) = self.stamps.get_mut(index) {
            *stamp = NEVER;
        }
    }

    /// Returns how many indices the set holds a stamp for.
    ///
    /// Visiting an index below this never grows the set. The count is zero until the first visit,
    /// whatever [`EpochBitset::with_capacity`] reserved.
    pub fn capacity(&self) -> usize {
        self.stamps.len()
    }

    /// Forgets every visit and returns to the first epoch, keeping the allocation.
    pub fn reset(&mut self) {
        self.stamps.fill(NEVER);
        self.epoch = NEVER + 1;
    }
}

impl Default for EpochBitset {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{EpochBitset, NEVER};

    #[test]
    fn the_epoch_that_would_wrap_clears_the_stamps_instead() {
        let mut bitset = EpochBitset::new();
        bitset.epoch = u32::MAX;
        assert!(bitset.visit(3));
        assert!(bitset.visit(9));

        bitset.bump();

        assert_eq!(bitset.epoch(), NEVER + 1);
        assert!(
            !bitset.contains(3),
            "a stamp from the last epoch was read as current"
        );
        assert!(!bitset.contains(9));
        assert!(bitset.stamps.iter().all(|stamp| *stamp == NEVER));
    }
}
