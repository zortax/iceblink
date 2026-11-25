//! One frame table: values by frame slot, with a journal of the slots that changed.

use iceblink::TableDelta;

use crate::tables::journal::{Coverage, Journal, TableVersion};

/// Values by frame slot, shared by every output.
///
/// A released slot is reused only once every output has built a frame after the release, so a
/// renderer never sees a slot change meaning while it holds a resident revision that refers to it.
#[derive(Debug)]
pub(crate) struct FrameTable<T> {
    values: Vec<T>,
    journal: Journal,
    free: Vec<u32>,
    /// Released slots and the build serial they were released at.
    quarantine: Vec<(u64, u32)>,
}

impl<T> Default for FrameTable<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            journal: Journal::default(),
            free: Vec::new(),
            quarantine: Vec::new(),
        }
    }
}

impl<T: Clone + PartialEq> FrameTable<T> {
    /// Stores `value` at a free slot and returns the slot.
    pub(crate) fn allocate(&mut self, value: T) -> u32 {
        let slot = match self.free.pop() {
            Some(slot) => {
                self.values[slot as usize] = value;
                slot
            }
            None => {
                self.values.push(value);
                (self.values.len() - 1) as u32
            }
        };

        self.journal.note(slot);
        slot
    }

    /// Replaces the value at `slot`, and journals the change if the value differs.
    pub(crate) fn write(&mut self, slot: u32, value: T) {
        let Some(held) = self.values.get_mut(slot as usize) else {
            return;
        };
        if *held != value {
            *held = value;
            self.journal.note(slot);
        }
    }

    /// Returns the value at `slot`.
    pub(crate) fn get(&self, slot: u32) -> Option<&T> {
        self.values.get(slot as usize)
    }

    /// Releases `slot`, which is reused once every output has built a frame after `serial`.
    pub(crate) fn release(&mut self, slot: u32, serial: u64) {
        if (slot as usize) < self.values.len() {
            self.quarantine.push((serial, slot));
        }
    }

    /// Makes reusable every slot released before build serial `settled`.
    pub(crate) fn settle(&mut self, settled: u64) {
        let free = &mut self.free;
        self.quarantine.retain(|&(serial, slot)| {
            let done = serial < settled;
            if done {
                free.push(slot);
            }
            !done
        });
    }

    /// Returns the version of the table as it is now.
    pub(crate) fn version(&self) -> TableVersion {
        self.journal.version()
    }

    /// Fills `delta` with the changes since `version`, or with the whole table when the journal
    /// cannot list them or `replace` is set.
    ///
    /// `slots` is emptied and then holds the changed slots.
    pub(crate) fn delta(
        &self,
        version: TableVersion,
        replace: bool,
        delta: &mut TableDelta<T>,
        slots: &mut Vec<u32>,
    ) {
        delta.clear();
        slots.clear();
        let coverage = if replace {
            Coverage::All
        } else {
            self.journal.changes_since(version, |slot| slots.push(slot))
        };

        match coverage {
            Coverage::All => {
                delta.replace = true;
                delta.all.extend_from_slice(&self.values);
            }
            Coverage::Delta => {
                slots.sort_unstable();
                slots.dedup();
                delta.changed.extend(
                    slots
                        .iter()
                        .filter_map(|&slot| Some((slot, self.values.get(slot as usize)?.clone()))),
                );
            }
        }
    }

    /// Returns the bytes the table's values occupy.
    pub(crate) fn bytes(&self) -> u64 {
        (self.values.capacity() * core::mem::size_of::<T>()) as u64
    }
}
