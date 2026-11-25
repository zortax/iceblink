//! The order nodes whose sequences hold each order node.

use crate::scene::parents::{MANY, Many};

/// The stored parent of a slot no sequence holds.
const NONE: u32 = u32::MAX;

/// The order nodes whose sequences hold each order node slot, once per entry.
///
/// A slot that one entry holds is looked up without hashing.
#[derive(Debug, Default)]
pub(crate) struct NodeParents {
    /// The parent of each slot, or `NONE` or `MANY`.
    one: Vec<u32>,
    many: Many<u32>,
}

impl NodeParents {
    /// Returns the nodes whose sequences hold `slot`, once per entry.
    #[inline]
    pub(crate) fn get(&self, slot: u32) -> &[u32] {
        match self.one.get(slot as usize) {
            None | Some(&NONE) => &[],
            Some(&MANY) => self.many.get(&slot),
            Some(parent) => core::slice::from_ref(parent),
        }
    }

    /// Returns the first node whose sequence holds `slot`.
    #[inline]
    pub(crate) fn first(&self, slot: u32) -> Option<u32> {
        self.get(slot).first().copied()
    }

    /// Reports whether a sequence holds `slot`.
    #[inline]
    pub(crate) fn contains(&self, slot: u32) -> bool {
        !self.get(slot).is_empty()
    }

    /// Notes that the sequence of `parent` holds `slot` once more, and reports whether sequences
    /// now hold it more than once.
    #[inline]
    pub(crate) fn place(&mut self, slot: u32, parent: u32) -> bool {
        let index = slot as usize;
        if index >= self.one.len() {
            self.one.resize(index + 1, NONE);
        }

        let held = self.one[index];
        if held == NONE {
            self.one[index] = parent;
            return false;
        }

        self.many.add(slot, held, parent);
        self.one[index] = MANY;
        true
    }

    /// Notes that the sequence of `parent` holds `slot` once less.
    #[inline]
    pub(crate) fn unplace(&mut self, slot: u32, parent: u32) {
        let Some(held) = self.one.get_mut(slot as usize) else {
            return;
        };
        if *held == MANY {
            *held = self.many.remove(&slot, parent).unwrap_or(NONE);
        } else if *held == parent {
            *held = NONE;
        }
    }
}
