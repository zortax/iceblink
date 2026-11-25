//! In-place access to one slot of a dense side table.

use core::marker::PhantomData;

use crate::arena::key::ArenaKey;

/// A view into one slot of a [`SlotVec`], which may be empty.
///
/// ```
/// use iceblink_alloc::arena::{ChunkArena, NamespaceId, SlotVec};
///
/// let mut arena: ChunkArena<&str> = ChunkArena::new(NamespaceId::FIRST);
/// let key = arena.insert("item");
///
/// let mut widths: SlotVec<_, Vec<u32>> = SlotVec::for_namespace(NamespaceId::FIRST);
/// widths.entry(key).or_default().push(3);
/// widths.entry(key).or_default().push(4);
/// assert_eq!(widths.get(key), Some(&vec![3, 4]));
/// ```
///
/// [`SlotVec`]: crate::arena::SlotVec
pub struct Entry<'a, K: ArenaKey, V> {
    slot: &'a mut Option<V>,
    /// The table's count of occupied slots.
    occupied: &'a mut usize,
    key: PhantomData<fn() -> K>,
}

impl<'a, K: ArenaKey, V> Entry<'a, K, V> {
    /// Wraps a slot and the table's count of occupied slots.
    pub(crate) fn new(slot: &'a mut Option<V>, occupied: &'a mut usize) -> Self {
        Self {
            slot,
            occupied,
            key: PhantomData,
        }
    }

    /// Reports whether the slot holds a value.
    pub fn is_occupied(&self) -> bool {
        self.slot.is_some()
    }

    /// Returns the slot's value, storing `value` first if the slot is empty.
    pub fn or_insert(self, value: V) -> &'a mut V {
        self.or_insert_with(|| value)
    }

    /// Returns the slot's value, storing the value `make` returns first if the slot is empty.
    pub fn or_insert_with(self, make: impl FnOnce() -> V) -> &'a mut V {
        if self.slot.is_none() {
            *self.occupied += 1;
            *self.slot = Some(make());
        }
        self.slot.as_mut().expect("just filled in")
    }

    /// Returns the slot's value, storing the default first if the slot is empty.
    pub fn or_default(self) -> &'a mut V
    where
        V: Default,
    {
        self.or_insert_with(V::default)
    }

    /// Stores `value` in the slot and returns the value it replaced.
    pub fn replace(self, value: V) -> Option<V> {
        match self.slot.replace(value) {
            Some(previous) => Some(previous),
            None => {
                *self.occupied += 1;
                None
            }
        }
    }

    /// Applies `change` to the slot's value if there is one, and returns the entry.
    pub fn and_modify(self, change: impl FnOnce(&mut V)) -> Self {
        if let Some(value) = self.slot.as_mut() {
            change(value);
        }
        self
    }
}
