//! The stable-address arena.

pub(crate) mod alloc;
pub(crate) mod block;
mod insert;
mod read;
mod remove;
mod succession;

use crate::arena::chunk::alloc::Slots;
use crate::arena::chunk::block::Block;
use crate::arena::key::NamespaceId;

pub use crate::arena::chunk::block::BLOCK_LEN;
pub use crate::arena::chunk::succession::Succession;

/// An arena whose values keep their address until they are dropped.
///
/// A reference returned by [`ChunkArena::get`] stays valid while other values are inserted and
/// removed.
///
/// Removal takes two steps:
///
/// 1. [`ChunkArena::remove`] marks a value removed. Its key keeps resolving.
/// 2. [`ChunkArena::recycle`] drops the removed values and frees their slots for reuse.
///
/// A slot is never reused before the next recycle, so a slot index keeps its meaning between two
/// recycles.
///
/// An arena is `Send` exactly when `T` is, and `Sync` exactly when `T` is.
///
/// ```
/// use iceblink_alloc::arena::{ChunkArena, NamespaceId};
///
/// let mut arena: ChunkArena<String> = ChunkArena::new(NamespaceId::FIRST);
/// let first = arena.insert("first".to_owned());
/// let second = arena.insert("second".to_owned());
///
/// assert!(arena.remove(first));
/// assert_eq!(arena.get(first).map(String::as_str), Some("first"));
///
/// arena.recycle();
/// assert_eq!(arena.get(first), None);
/// assert_eq!(arena.get(second).map(String::as_str), Some("second"));
/// ```
pub struct ChunkArena<T> {
    /// Storage blocks, appended and never moved.
    blocks: Vec<Box<Block<T>>>,
    slots: Slots,
    namespace: NamespaceId,
}

impl<T> ChunkArena<T> {
    /// Creates an empty arena that issues keys in `namespace`.
    ///
    /// Nothing is allocated until the first insertion.
    pub const fn new(namespace: NamespaceId) -> Self {
        Self {
            blocks: Vec::new(),
            slots: Slots::new(),
            namespace,
        }
    }

    /// Returns the namespace of the keys the arena issues.
    pub const fn namespace(&self) -> NamespaceId {
        self.namespace
    }

    /// Returns the number of live values.
    ///
    /// A removed value is not live, even before the recycle that drops it.
    pub const fn len(&self) -> u32 {
        self.slots.live()
    }

    /// Reports whether no value is live.
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of slots, including vacant and retired ones.
    ///
    /// The capacity never decreases.
    pub fn capacity(&self) -> u32 {
        self.slots.capacity()
    }

    /// Returns the number of retired slots.
    ///
    /// A slot is retired once its occupant with
    /// [`Generation::LAST`](crate::arena::Generation::LAST) is dropped, and is never reused.
    pub fn retired(&self) -> u32 {
        self.slots.retired()
    }
}

impl<T> core::fmt::Debug for ChunkArena<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ChunkArena")
            .field("namespace", &self.namespace)
            .field("len", &self.len())
            .field("capacity", &self.capacity())
            .field("retired", &self.retired())
            .finish()
    }
}
