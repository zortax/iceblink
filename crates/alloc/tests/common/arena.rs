//! Builders shared by the arena tests.

use std::cell::Cell;
use std::rc::Rc;

use iceblink_alloc::arena::{Generation, Key, NamespaceId};

/// A key of the first namespace at the first generation.
pub(crate) fn key(index: u32) -> Key<()> {
    Key::new(index, Generation::FIRST, NamespaceId::FIRST)
}

/// A value that counts its drops.
pub(crate) struct Tally(pub(crate) Rc<Cell<u32>>);

impl Drop for Tally {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
