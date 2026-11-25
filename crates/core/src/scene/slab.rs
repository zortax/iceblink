//! Objects of one kind, stored at the slots of their ids.

/// The objects of one kind, indexed by the slot of their id.
///
/// Each slot remembers the highest generation it has held, so a create can be refused when its
/// generation is not newer than everything the slot held before.
#[derive(Debug)]
pub(crate) struct Slab<T> {
    slots: Vec<Slot<T>>,
}

/// One slot: its current or last occupant's generation, the highest generation it has held, and
/// the occupant.
#[derive(Debug)]
struct Slot<T> {
    generation: u32,
    highest: u32,
    value: Option<T>,
}

/// What a slot held before a create, so the create can be undone.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Vacancy {
    generation: u32,
    highest: u32,
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> Slab<T> {
    /// Returns the object at `slot` if its generation is `generation`.
    pub(crate) fn get(&self, slot: u32, generation: u32) -> Option<&T> {
        let held = self.slots.get(slot as usize)?;
        held.value
            .as_ref()
            .filter(|_| held.generation == generation)
    }

    /// Returns the object at `slot` mutably if its generation is `generation`.
    pub(crate) fn get_mut(&mut self, slot: u32, generation: u32) -> Option<&mut T> {
        let held = self.slots.get_mut(slot as usize)?;
        if held.generation != generation {
            return None;
        }
        held.value.as_mut()
    }

    /// Returns the object at `slot`, whatever its generation.
    pub(crate) fn at(&self, slot: u32) -> Option<&T> {
        self.slots.get(slot as usize)?.value.as_ref()
    }

    /// Returns the object at `slot` mutably, whatever its generation.
    pub(crate) fn at_mut(&mut self, slot: u32) -> Option<&mut T> {
        self.slots.get_mut(slot as usize)?.value.as_mut()
    }

    /// Reports whether a create at `slot` with `generation` is allowed.
    ///
    /// The slot must be vacant, and `generation` must be greater than every generation it held.
    pub(crate) fn can_create(&self, slot: u32, generation: u32) -> bool {
        match self.slots.get(slot as usize) {
            Some(held) => held.value.is_none() && generation > held.highest,
            None => generation > 0,
        }
    }

    /// Puts `value` at a vacant slot and returns what the slot held before.
    pub(crate) fn create(&mut self, slot: u32, generation: u32, value: T) -> Vacancy {
        let index = slot as usize;
        if index >= self.slots.len() {
            self.slots.resize_with(index + 1, || Slot {
                generation: 0,
                highest: 0,
                value: None,
            });
        }

        let held = &mut self.slots[index];
        let vacancy = Vacancy {
            generation: held.generation,
            highest: held.highest,
        };
        held.generation = generation;
        held.highest = held.highest.max(generation);
        held.value = Some(value);
        vacancy
    }

    /// Undoes the create at `slot` that returned `vacancy`, and returns the object it put there.
    pub(crate) fn uncreate(&mut self, slot: u32, vacancy: Vacancy) -> Option<T> {
        let held = self.slots.get_mut(slot as usize)?;

        held.generation = vacancy.generation;
        held.highest = vacancy.highest;
        held.value.take()
    }

    /// Takes the object out of `slot`, keeping the slot's generations.
    pub(crate) fn remove(&mut self, slot: u32) -> Option<T> {
        self.slots.get_mut(slot as usize)?.value.take()
    }

    /// Puts a removed object back into its slot.
    pub(crate) fn restore(&mut self, slot: u32, value: T) {
        if let Some(held) = self.slots.get_mut(slot as usize) {
            held.value = Some(value);
        }
    }

    /// Calls `visit` with the slot, generation and object of every live slot, in slot order.
    pub(crate) fn each(&self, mut visit: impl FnMut(u32, u32, &T)) {
        for (slot, held) in self.slots.iter().enumerate() {
            if let Some(value) = &held.value {
                visit(slot as u32, held.generation, value);
            }
        }
    }

    /// Returns how many slots the slab spans, live or vacant.
    pub(crate) fn span(&self) -> usize {
        self.slots.len()
    }
}
