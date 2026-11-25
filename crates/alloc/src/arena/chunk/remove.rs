//! Removing, taking and dropping values.

// Moving or dropping a slot's value is a raw-memory operation from `block`.
#![allow(unsafe_code)]

use crate::arena::chunk::ChunkArena;
use crate::arena::chunk::alloc::SlotState;
use crate::arena::chunk::block::BLOCK_LEN;
use crate::arena::key::Key;

impl<T> ChunkArena<T> {
    /// Marks the value `key` refers to as removed.
    ///
    /// The value stays readable through its key until the next [`ChunkArena::recycle`].
    ///
    /// Returns whether the key referred to a live value.
    pub fn remove(&mut self, key: Key<T>) -> bool {
        match self.resolve(key) {
            Some(index) => self.slots.remove(index),
            None => false,
        }
    }

    /// Removes the value `key` refers to and returns it.
    ///
    /// The key stops resolving immediately. The slot is reused only after the next
    /// [`ChunkArena::recycle`].
    pub fn take(&mut self, key: Key<T>) -> Option<T> {
        let index = self.resolve(key)?;
        // SAFETY: the key resolved, so the slot holds a value. `&mut self` rules out any live
        // reference into it, and `vacate` records the slot as empty directly below, so the value
        // is never read again.
        let value =
            unsafe { self.blocks[index as usize / BLOCK_LEN].take(index as usize % BLOCK_LEN) };
        self.slots.vacate(index);
        Some(value)
    }

    /// Drops the removed values and frees their slots for reuse.
    ///
    /// Keys to the dropped values stop resolving. Slots emptied by [`ChunkArena::take`] are freed
    /// for reuse as well.
    pub fn recycle(&mut self) {
        let pending = self.slots.take_pending();
        for &index in &pending {
            match self.slots.state(index) {
                SlotState::Removed => {
                    // SAFETY: a removed slot holds a value that has not been dropped, and
                    // `&mut self` rules out any live reference into it. `release` records the
                    // slot as empty directly below, so the value is never dropped twice.
                    unsafe {
                        self.blocks[index as usize / BLOCK_LEN]
                            .drop_value(index as usize % BLOCK_LEN);
                    }
                    self.slots.release(index);
                }
                // `take` already moved the value out and advanced the generation.
                SlotState::Vacant => self.slots.offer(index),
                SlotState::Live => unreachable!("a slot awaiting recycling is never live"),
            }
        }
        self.slots.restore_pending(pending);
    }
}

impl<T> Drop for ChunkArena<T> {
    fn drop(&mut self) {
        if !core::mem::needs_drop::<T>() {
            return;
        }
        for index in 0..self.slots.capacity() {
            match self.slots.state(index) {
                SlotState::Live | SlotState::Removed => {
                    // SAFETY: both states mean the slot holds a value that has not been dropped.
                    // The arena is being destroyed, so no reference into it can be live, and each
                    // slot index is visited exactly once.
                    unsafe {
                        self.blocks[index as usize / BLOCK_LEN]
                            .drop_value(index as usize % BLOCK_LEN);
                    }
                }
                SlotState::Vacant => {}
            }
        }
    }
}
