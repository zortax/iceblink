//! A renderer's copy of one frame table.

use std::collections::BTreeMap;

use iceblink::TableDelta;

use crate::validate::known::Lookup;

/// A renderer's copy of one frame table, as far as the frames seen so far tell it.
#[derive(Clone, Debug)]
pub(crate) enum Table<T> {
    /// Every slot is known. A slot that a delta skipped over holds `None`.
    Complete(Vec<Option<T>>),
    /// Only the slots written since the validator joined the output are known.
    Partial(BTreeMap<u32, T>),
}

impl<T: Clone> Table<T> {
    /// Applies `delta` by the rules of `TableDelta::apply`.
    pub(crate) fn apply(&mut self, delta: &TableDelta<T>) {
        if delta.replace {
            *self = Self::Complete(delta.all.iter().cloned().map(Some).collect());
        }

        match self {
            Self::Complete(slots) => {
                for (slot, value) in &delta.changed {
                    let slot = *slot as usize;
                    if slot >= slots.len() {
                        slots.resize(slot + 1, None);
                    }
                    slots[slot] = Some(value.clone());
                }
            }
            Self::Partial(slots) => {
                for (slot, value) in &delta.changed {
                    slots.insert(*slot, value.clone());
                }
            }
        }
    }

    /// Returns what is known about `slot`.
    pub(crate) fn get(&self, slot: u32) -> Lookup<&T> {
        match self {
            Self::Complete(slots) => match slots.get(slot as usize) {
                Some(Some(value)) => Lookup::Found(value),
                Some(None) | None => Lookup::Missing,
            },
            Self::Partial(slots) => slots.get(&slot).map_or(Lookup::Unknown, Lookup::Found),
        }
    }
}
