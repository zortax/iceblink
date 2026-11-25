//! Borrowing several entries of a sparse side table together.

use crate::arena::key::ArenaKey;
use crate::arena::paged_vec::page::Page;
use crate::arena::paged_vec::{PagedVec, split};

impl<K: ArenaKey, V: Default, const N: usize> PagedVec<K, V, N> {
    /// Returns the values stored for `keys` for modification together, in the same order.
    ///
    /// # Panics
    ///
    /// Panics if the keys are not in strictly ascending index order, or if the page of a key is not
    /// allocated.
    pub fn disjoint_mut(&mut self, keys: &[K]) -> Vec<&mut V> {
        assert!(
            keys.windows(2)
                .all(|pair| pair[0].index() < pair[1].index()),
            "disjoint borrows need strictly ascending keys"
        );

        // Every borrowed entry may be written.
        for &key in keys {
            self.check(key);
            let (page, _) = split::<K, N>(key);
            self.written.mark(page);
        }

        let mut out = Vec::with_capacity(keys.len());
        let mut rest: &mut [Option<Page<V, N>>] = &mut self.pages;
        let mut next_page = 0usize;
        let mut index = 0;
        while index < keys.len() {
            let (page, _) = split::<K, N>(keys[index]);
            let (_, tail) = rest.split_at_mut(page - next_page);
            let (entry_page, tail) = tail
                .split_first_mut()
                .expect("every borrowed key is within the table");
            rest = tail;
            next_page = page + 1;

            let mut slots: &mut [V] = entry_page
                .as_mut()
                .expect("every borrowed key has a written page")
                .slice_mut();
            let mut next_slot = 0usize;
            while index < keys.len() {
                let (entry_page_index, slot) = split::<K, N>(keys[index]);
                if entry_page_index != page {
                    break;
                }
                let (_, tail) = slots.split_at_mut(slot - next_slot);
                let (entry, tail) = tail.split_first_mut().expect("a slot is within its page");
                slots = tail;
                next_slot = slot + 1;
                out.push(entry);
                index += 1;
            }
        }
        out
    }
}
