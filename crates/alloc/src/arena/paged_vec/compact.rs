//! Dropping pages that hold only defaults.

use crate::arena::key::ArenaKey;
use crate::arena::paged_vec::PagedVec;

impl<K: ArenaKey, V, const N: usize> PagedVec<K, V, N> {
    /// Drops every page written since the last compaction whose entries all satisfy `is_default`.
    ///
    /// Dropping a page resets its entries to the default, so `is_default` must hold only for
    /// values equal to the default.
    pub fn compact_by(&mut self, is_default: impl Fn(&V) -> bool) {
        let mut dropped = false;
        for index in self.written.drain() {
            let Some(page) = self.pages.get_mut(index) else {
                continue;
            };
            if page
                .as_ref()
                .is_some_and(|entries| entries.iter().all(&is_default))
            {
                *page = None;
                dropped = true;
            }
        }
        if !dropped {
            return;
        }

        let kept = self.pages.iter().rposition(Option::is_some);
        self.pages.truncate(kept.map_or(0, |index| index + 1));
        self.pages.shrink_to_fit();
    }
}

impl<K: ArenaKey, V: Default + PartialEq, const N: usize> PagedVec<K, V, N> {
    /// Drops every page written since the last compaction whose entries all equal the default.
    pub fn compact(&mut self) {
        let default = V::default();
        self.compact_by(|entry| *entry == default);
    }
}
