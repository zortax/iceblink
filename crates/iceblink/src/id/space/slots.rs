//! The indices and generations of one id kind.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The first index no id may use.
// Items reserve the values from 2³¹ upwards.
const INDEX_LIMIT: u32 = 1 << 31;

/// The state of every index of one kind ever minted.
#[derive(Debug, Default)]
pub(super) struct Slots {
    slots: Vec<Slot>,
    free: BinaryHeap<Reverse<u32>>,
}

/// One index: the generation it mints next or holds now, and whether it is free.
#[derive(Debug)]
struct Slot {
    generation: u32,
    free: bool,
}

impl Slots {
    /// Mints the lowest free index, or one past the highest index ever minted.
    ///
    /// # Panics
    ///
    /// Panics if every index below 2³¹ is live or retired.
    pub(super) fn mint(&mut self) -> (u32, u32) {
        if let Some(Reverse(index)) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.free = false;
            return (index, slot.generation);
        }

        let index = u32::try_from(self.slots.len())
            .ok()
            .filter(|&index| index < INDEX_LIMIT)
            .expect("id space exhausted: every index below 2^31 is live or retired");
        self.slots.push(Slot {
            generation: 1,
            free: false,
        });
        (index, 1)
    }

    /// Frees `index` if `generation` is its current one.
    ///
    /// The index then mints with the next generation, or is retired if `generation` is `u32::MAX`.
    pub(super) fn free(&mut self, index: u32, generation: u32) {
        let Some(slot) = self.slots.get_mut(index as usize) else {
            return;
        };
        if slot.free || slot.generation != generation {
            return;
        }

        // An index whose generation cannot grow is retired, so no stale id ever matches it again.
        let Some(next) = generation.checked_add(1) else {
            return;
        };
        slot.generation = next;
        slot.free = true;
        self.free.push(Reverse(index));
    }
}
