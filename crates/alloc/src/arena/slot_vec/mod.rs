//! The dense side table.

mod entry;

use core::marker::PhantomData;

use crate::arena::key::{ArenaKey, NamespaceId};

pub use crate::arena::slot_vec::entry::Entry;

/// A dense side table with at most one value per arena slot.
///
/// Values are stored in one vector indexed by slot, up to the highest slot written. A table created
/// with [`SlotVec::for_namespace`] panics in debug builds when given a key of another namespace.
///
/// ```
/// use iceblink_alloc::arena::{ChunkArena, NamespaceId, SlotVec};
///
/// let mut arena: ChunkArena<&str> = ChunkArena::new(NamespaceId::FIRST);
/// let key = arena.insert("item");
///
/// let mut depths: SlotVec<_, u32> = SlotVec::for_namespace(arena.namespace());
/// depths.insert(key, 3);
/// assert_eq!(depths.get(key), Some(&3));
/// assert_eq!(depths.remove(key), Some(3));
/// assert_eq!(depths.get(key), None);
/// ```
pub struct SlotVec<K: ArenaKey, V> {
    slots: Vec<Option<V>>,
    /// The number of slots that hold a value.
    occupied: usize,
    /// The namespace keys are checked against, if any.
    namespace: Option<NamespaceId>,
    key: PhantomData<fn() -> K>,
}

impl<K: ArenaKey, V> SlotVec<K, V> {
    /// Creates an empty table that accepts keys of every namespace.
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            occupied: 0,
            namespace: None,
            key: PhantomData,
        }
    }

    /// Creates an empty table for the keys of `namespace`.
    pub const fn for_namespace(namespace: NamespaceId) -> Self {
        Self {
            slots: Vec::new(),
            occupied: 0,
            namespace: Some(namespace),
            key: PhantomData,
        }
    }

    /// Returns the number of stored values.
    pub const fn len(&self) -> usize {
        self.occupied
    }

    /// Reports whether no value is stored.
    pub const fn is_empty(&self) -> bool {
        self.occupied == 0
    }

    /// Returns the value stored for `key`.
    pub fn get(&self, key: K) -> Option<&V> {
        self.check(key);
        self.slots.get(key.index() as usize)?.as_ref()
    }

    /// Returns the value stored for `key` for modification.
    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.check(key);
        self.slots.get_mut(key.index() as usize)?.as_mut()
    }

    /// Reports whether a value is stored for `key`.
    pub fn contains_key(&self, key: K) -> bool {
        self.get(key).is_some()
    }

    /// Stores `value` for `key` and returns the value it replaced.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.entry(key).replace(value)
    }

    /// Removes the value stored for `key` and returns it.
    pub fn remove(&mut self, key: K) -> Option<V> {
        self.check(key);
        let previous = self.slots.get_mut(key.index() as usize)?.take();
        if previous.is_some() {
            self.occupied -= 1;
        }
        previous
    }

    /// Returns the entry for the slot of `key`.
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        self.check(key);
        let index = key.index() as usize;
        if index >= self.slots.len() {
            self.slots.resize_with(index + 1, || None);
        }
        Entry::new(&mut self.slots[index], &mut self.occupied)
    }

    /// Removes every value and keeps the allocation.
    pub fn clear(&mut self) {
        self.slots.clear();
        self.occupied = 0;
    }

    /// Returns every stored value with its slot index, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &V)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| Some((index as u32, slot.as_ref()?)))
    }

    /// Returns every stored value with its slot index for modification, in slot order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &mut V)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| Some((index as u32, slot.as_mut()?)))
    }

    /// Panics in debug builds if `key` belongs to another namespace.
    #[inline]
    fn check(&self, key: K) {
        debug_assert!(
            self.namespace
                .is_none_or(|namespace| namespace == key.namespace()),
            "a key from another namespace was used on a side table"
        );
    }
}

impl<K: ArenaKey, V> Default for SlotVec<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: ArenaKey, V: core::fmt::Debug> core::fmt::Debug for SlotVec<K, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
