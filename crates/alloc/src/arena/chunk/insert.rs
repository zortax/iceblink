//! Inserting values.

// Writing a value into a claimed slot is a raw-memory operation from `block`.
#![allow(unsafe_code)]

use crate::arena::chunk::ChunkArena;
use crate::arena::chunk::alloc::Slots;
use crate::arena::chunk::block::{BLOCK_LEN, Block};
use crate::arena::key::Key;

impl<T> ChunkArena<T> {
    /// Stores a value and returns its key.
    ///
    /// # Panics
    ///
    /// Panics if the arena has run out of slot indices.
    pub fn insert(&mut self, value: T) -> Key<T> {
        self.insert_with(|_| value)
    }

    /// Stores the value `make` builds from its future key, and returns that key.
    ///
    /// If `make` panics, nothing is stored.
    ///
    /// # Panics
    ///
    /// Panics if the arena has run out of slot indices.
    pub fn insert_with(&mut self, make: impl FnOnce(Key<T>) -> T) -> Key<T> {
        let (index, generation) = self.slots.allocate();
        let key = Key::new(index, generation, self.namespace);

        let value = {
            // Empties the slot again if `make` unwinds, so nothing drops the unwritten slot.
            let claimed = Claimed {
                slots: &mut self.slots,
                index,
            };
            let value = make(key);
            core::mem::forget(claimed);
            value
        };

        // A successor arena can start past a whole block of retired slots, so more than one block
        // may be missing.
        let block = index as usize / BLOCK_LEN;
        while self.blocks.len() <= block {
            self.blocks.push(Block::new());
        }

        // SAFETY: the slot was just claimed, so it holds no value and nothing refers to it. A slot
        // only leaves circulation through `recycle` or `take`, both of which empty it first.
        unsafe { self.blocks[block].write(index as usize % BLOCK_LEN, value) };
        key
    }
}

/// A claimed slot whose value is still being built, vacated again on drop.
struct Claimed<'slots> {
    slots: &'slots mut Slots,
    index: u32,
}

impl Drop for Claimed<'_> {
    fn drop(&mut self) {
        self.slots.vacate(self.index);
    }
}
