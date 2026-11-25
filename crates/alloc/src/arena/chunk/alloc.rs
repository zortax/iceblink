//! Slot bookkeeping: each slot's state and generation, and the slots ready for reuse.

use crate::arena::key::Generation;

/// The state of one slot.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum SlotState {
    /// The slot holds no value.
    Vacant,
    /// The slot holds a live value.
    Live,
    /// The slot holds a removed value that is not yet dropped.
    Removed,
}

/// The state and generation of every slot, and the lists of slots awaiting reuse.
///
/// A slot emptied since the last recycle is pending and is not reused. A slot whose generation runs
/// out is retired and is never reused.
pub(crate) struct Slots {
    /// The generation of each slot, or `None` once the slot is retired.
    generations: Vec<Option<Generation>>,
    states: Vec<SlotState>,
    /// Slots emptied since the last recycle.
    pending: Vec<u32>,
    /// Slots ready for reuse.
    free: Vec<u32>,
    live: u32,
    retired: u32,
}

impl Slots {
    /// Creates a table with no slots.
    pub(crate) const fn new() -> Self {
        Self {
            generations: Vec::new(),
            states: Vec::new(),
            pending: Vec::new(),
            free: Vec::new(),
            live: 0,
            retired: 0,
        }
    }

    /// Returns the number of live slots.
    pub(crate) const fn live(&self) -> u32 {
        self.live
    }

    /// Returns the number of retired slots.
    pub(crate) const fn retired(&self) -> u32 {
        self.retired
    }

    /// Returns the number of slots in any state.
    pub(crate) fn capacity(&self) -> u32 {
        self.states.len() as u32
    }

    /// Creates a table of vacant slots at the given generations.
    ///
    /// A slot whose generation is `None` is retired. The lowest free slot is reused first.
    pub(crate) fn succeeding(generations: &[Option<Generation>]) -> Self {
        let free: Vec<u32> = (0..generations.len() as u32)
            .rev()
            .filter(|index| generations[*index as usize].is_some())
            .collect();
        let retired = generations.len() as u32 - free.len() as u32;
        Self {
            generations: generations.to_vec(),
            states: vec![SlotState::Vacant; generations.len()],
            pending: Vec::new(),
            free,
            live: 0,
            retired,
        }
    }

    /// Returns the generation of each slot's next occupant, counting every value as dropped.
    pub(crate) fn successors(&self) -> Vec<Option<Generation>> {
        self.generations
            .iter()
            .zip(&self.states)
            .map(|(generation, state)| match state {
                // A vacant slot already carries the generation of its next occupant.
                SlotState::Vacant => *generation,
                SlotState::Live | SlotState::Removed => generation.and_then(Generation::next),
            })
            .collect()
    }

    /// Returns the state of an existing slot.
    pub(crate) fn state(&self, index: u32) -> SlotState {
        self.states[index as usize]
    }

    /// Claims a slot for a new value, reusing a free slot if there is one.
    ///
    /// # Panics
    ///
    /// Panics if the table has run out of slot indices.
    pub(crate) fn allocate(&mut self) -> (u32, Generation) {
        self.live += 1;
        if let Some(index) = self.free.pop() {
            self.states[index as usize] = SlotState::Live;
            let generation = self.generations[index as usize]
                .expect("a retired slot never reaches the free list");
            return (index, generation);
        }
        let index = u32::try_from(self.states.len()).expect("arena slot indices exhausted");
        self.generations.push(Some(Generation::FIRST));
        self.states.push(SlotState::Live);
        (index, Generation::FIRST)
    }

    /// Returns the slot a key refers to, if the slot holds the value the key was issued for.
    pub(crate) fn resolve(&self, index: u32, generation: Generation) -> Option<u32> {
        if self.generations.get(index as usize).copied().flatten()? != generation {
            return None;
        }
        // A vacant slot already carries its next occupant's generation, which a hand-built key
        // can guess.
        if self.states[index as usize] == SlotState::Vacant {
            return None;
        }
        Some(index)
    }

    /// Marks a live slot removed and returns whether it was live.
    pub(crate) fn remove(&mut self, index: u32) -> bool {
        if self.states[index as usize] != SlotState::Live {
            return false;
        }
        self.states[index as usize] = SlotState::Removed;
        self.pending.push(index);
        self.live -= 1;
        true
    }

    /// Marks a slot vacant after its value was moved out, and advances its generation.
    ///
    /// The slot becomes reusable after the next recycle.
    pub(crate) fn vacate(&mut self, index: u32) {
        let already_pending = self.states[index as usize] == SlotState::Removed;
        if self.states[index as usize] == SlotState::Live {
            self.live -= 1;
        }
        self.states[index as usize] = SlotState::Vacant;
        self.advance(index);
        if !already_pending {
            self.pending.push(index);
        }
    }

    /// Takes the slots emptied since the last call.
    pub(crate) fn take_pending(&mut self) -> Vec<u32> {
        core::mem::take(&mut self.pending)
    }

    /// Reuses the allocation of a list taken by [`Slots::take_pending`].
    pub(crate) fn restore_pending(&mut self, mut pending: Vec<u32>) {
        pending.clear();
        if self.pending.is_empty() {
            self.pending = pending;
        }
    }

    /// Marks a removed slot vacant after its value was dropped, and frees it for reuse.
    pub(crate) fn release(&mut self, index: u32) {
        debug_assert_eq!(self.states[index as usize], SlotState::Removed);
        self.states[index as usize] = SlotState::Vacant;
        self.advance(index);
        self.offer(index);
    }

    /// Frees a vacant slot for reuse, unless it is retired.
    pub(crate) fn offer(&mut self, index: u32) {
        if self.generations[index as usize].is_some() {
            self.free.push(index);
        }
    }

    /// Advances a slot to its next generation, retiring it after [`Generation::LAST`].
    fn advance(&mut self, index: u32) {
        let generation = &mut self.generations[index as usize];
        let next = generation.and_then(Generation::next);
        if next.is_none() && generation.is_some() {
            self.retired += 1;
        }
        *generation = next;
    }
}
