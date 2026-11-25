//! Writing entries of a sparse side table.

use crate::arena::key::ArenaKey;
use crate::arena::paged_vec::page::Page;
use crate::arena::paged_vec::{PagedVec, split};

impl<K: ArenaKey, V: Default, const N: usize> PagedVec<K, V, N> {
    /// Returns the value stored for `key` for modification, allocating its page if needed.
    pub fn get_mut(&mut self, key: K) -> &mut V {
        self.check(key);
        let (page, slot) = split::<K, N>(key);
        if page >= self.pages.len() {
            self.pages.resize_with(page + 1, || None);
        }
        // Writing the default through this borrow clears the entry, so it counts as a write.
        self.written.mark(page);
        self.pages[page].get_or_insert_with(Page::new).get_mut(slot)
    }

    /// Stores `value` for `key` and returns the value it replaced, allocating the page if needed.
    pub fn replace(&mut self, key: K, value: V) -> V {
        core::mem::replace(self.get_mut(key), value)
    }

    /// Resets the value stored for `key` to the default.
    ///
    /// Nothing is allocated for a slot on an unallocated page.
    pub fn clear(&mut self, key: K) {
        self.check(key);
        let (index, slot) = split::<K, N>(key);
        if let Some(Some(page)) = self.pages.get_mut(index) {
            *page.get_mut(slot) = V::default();
            self.written.mark(index);
        }
    }
}
