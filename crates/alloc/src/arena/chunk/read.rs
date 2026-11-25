//! Reading values through their keys.

// Borrowing a slot's value is a raw-memory operation from `block`.
#![allow(unsafe_code)]

use crate::arena::chunk::ChunkArena;
use crate::arena::chunk::alloc::SlotState;
use crate::arena::chunk::block::BLOCK_LEN;
use crate::arena::key::Key;

impl<T> ChunkArena<T> {
    /// Returns the value `key` refers to, if the key resolves.
    ///
    /// A key resolves from its insertion until [`ChunkArena::recycle`] or [`ChunkArena::take`]
    /// drops its value. A key of another namespace, or with an index or generation the arena never
    /// issued, does not resolve.
    pub fn get(&self, key: Key<T>) -> Option<&T> {
        let index = self.resolve(key)?;
        // SAFETY: the key resolved, so the slot holds a value, and the slot cannot be emptied
        // while the returned reference lives: every operation that empties a slot takes
        // `&mut self`, which the borrow of `self` behind this reference rules out.
        Some(unsafe { self.blocks[index as usize / BLOCK_LEN].get(index as usize % BLOCK_LEN) })
    }

    /// Returns the value `key` refers to for modification, if the key resolves.
    pub fn get_mut(&mut self, key: Key<T>) -> Option<&mut T> {
        let index = self.resolve(key)?;
        // SAFETY: the key resolved, so the slot holds a value, and `&mut self` is proof that no
        // other reference into the arena is live for as long as the returned one is.
        Some(unsafe { self.blocks[index as usize / BLOCK_LEN].get_mut(index as usize % BLOCK_LEN) })
    }

    /// Reports whether `key` resolves.
    pub fn contains_key(&self, key: Key<T>) -> bool {
        self.get(key).is_some()
    }

    /// Returns every value whose key resolves, in slot order.
    ///
    /// Removed values are included until they are recycled.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        (0..self.slots.capacity()).filter_map(|index| match self.slots.state(index) {
            // SAFETY: the slot's state says it holds a value, and the slot cannot be emptied
            // while the returned reference lives: every operation that empties a slot takes
            // `&mut self`, which the borrow of `self` behind this reference rules out.
            SlotState::Live | SlotState::Removed => Some(unsafe {
                self.blocks[index as usize / BLOCK_LEN].get(index as usize % BLOCK_LEN)
            }),
            SlotState::Vacant => None,
        })
    }

    /// Returns the slot `key` refers to, if the key resolves.
    pub(crate) fn resolve(&self, key: Key<T>) -> Option<u32> {
        if key.namespace() != self.namespace {
            return None;
        }
        self.slots.resolve(key.index(), key.generation())
    }
}
