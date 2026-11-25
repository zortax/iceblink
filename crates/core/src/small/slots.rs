//! A set of slots, one bit each.

/// A set of slots, which holds a bit for every slot up to the highest one added.
#[derive(Debug, Default)]
pub(crate) struct SlotSet {
    words: Vec<u64>,
}

impl SlotSet {
    /// Returns the set of `slots`.
    pub(crate) fn of(slots: &[u32]) -> Self {
        let span = slots.iter().max().map_or(0, |&slot| slot as usize / 64 + 1);
        let mut words = vec![0; span];
        for &slot in slots {
            words[slot as usize / 64] |= 1 << (slot % 64);
        }
        Self { words }
    }

    /// Adds `slot`, growing the set to hold it.
    pub(crate) fn insert(&mut self, slot: u32) {
        let word = slot as usize / 64;
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        self.words[word] |= 1 << (slot % 64);
    }

    /// Takes `slot` out of the set.
    pub(crate) fn remove(&mut self, slot: u32) {
        if let Some(word) = self.words.get_mut(slot as usize / 64) {
            *word &= !(1 << (slot % 64));
        }
    }

    pub(crate) fn contains(&self, slot: u32) -> bool {
        self.words
            .get(slot as usize / 64)
            .is_some_and(|word| word & (1 << (slot % 64)) != 0)
    }
}
