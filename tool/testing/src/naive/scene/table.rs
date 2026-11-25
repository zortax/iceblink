//! A client's objects of one kind, by slot.

use iceblink::{Id, RejectReason};

/// The objects of one kind, indexed by slot, with the generation each slot holds.
#[derive(Clone, Debug)]
pub(crate) struct Table<T> {
    slots: Vec<Slot<T>>,
}

#[derive(Clone, Debug)]
struct Slot<T> {
    /// The highest generation the slot has held.
    held: u32,
    /// The object, while the slot is live.
    live: Option<T>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> Table<T> {
    /// Returns the object stored under `id`, if it is live with the id's generation.
    pub(crate) fn get<K>(&self, id: Id<K>) -> Option<&T> {
        let slot = self.slots.get(id.slot() as usize)?;
        if slot.held == id.generation() {
            slot.live.as_ref()
        } else {
            None
        }
    }

    /// Returns the object stored under `id` for writing, if it is live with the id's generation.
    pub(crate) fn get_mut<K>(&mut self, id: Id<K>) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.slot() as usize)?;
        if slot.held == id.generation() {
            slot.live.as_mut()
        } else {
            None
        }
    }

    /// Returns the live object at `slot`, whatever its generation.
    pub(crate) fn at(&self, slot: u32) -> Option<&T> {
        self.slots.get(slot as usize)?.live.as_ref()
    }

    /// Returns the generation of the live object at `slot`.
    pub(crate) fn generation_at(&self, slot: u32) -> Option<u32> {
        let held = self.slots.get(slot as usize)?;
        held.live.as_ref().map(|_| held.held)
    }

    /// Reports whether `id` refers to a live object with its generation.
    pub(crate) fn is_live<K>(&self, id: Id<K>) -> bool {
        self.get(id).is_some()
    }

    /// Reports whether a create may store a value under `id`: its slot is not live, and its
    /// generation is above every generation the slot has held.
    pub(crate) fn can_create<K>(&self, id: Id<K>) -> bool {
        self.slots
            .get(id.slot() as usize)
            .is_none_or(|slot| slot.live.is_none() && id.generation() > slot.held)
    }

    /// Stores `value` under a new id.
    ///
    /// Fails with `StaleId` if the slot is live or the id's generation is not above every
    /// generation the slot has held.
    pub(crate) fn create<K>(&mut self, id: Id<K>, value: T) -> Result<(), RejectReason> {
        let index = id.slot() as usize;
        if index >= self.slots.len() {
            self.slots.resize_with(index + 1, || Slot {
                held: 0,
                live: None,
            });
        }

        let slot = &mut self.slots[index];
        if slot.live.is_some() || id.generation() <= slot.held {
            return Err(RejectReason::StaleId);
        }

        slot.held = id.generation();
        slot.live = Some(value);
        Ok(())
    }

    /// Removes the object stored under `id` and returns it.
    pub(crate) fn remove<K>(&mut self, id: Id<K>) -> Option<T> {
        let slot = self.slots.get_mut(id.slot() as usize)?;
        if slot.held == id.generation() {
            slot.live.take()
        } else {
            None
        }
    }

    /// Returns the slot, generation and value of every live object, by slot.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, u32, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            let value = slot.live.as_ref()?;
            Some((index as u32, slot.held, value))
        })
    }
}
