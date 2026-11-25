//! One page of a sparse side table.

/// The default number of slots per page of a [`PagedVec`](crate::arena::PagedVec).
// A page of a 16-byte value is 16 KiB.
pub const PAGE_LEN: usize = 1024;

/// A run of `N` entries in one allocation.
pub(crate) struct Page<V, const N: usize>(Box<[V; N]>);

impl<V: Default, const N: usize> Page<V, N> {
    /// Allocates a page with every entry at the default.
    pub(crate) fn new() -> Self {
        assert!(N > 0, "a sparse-table page must hold at least one entry");
        // Built in the heap allocation, so a large page never passes through the stack.
        let mut entries = Vec::new();
        entries.resize_with(N, V::default);
        match entries.into_boxed_slice().try_into() {
            Ok(entries) => Self(entries),
            Err(_) => unreachable!("the vector was filled to exactly one page"),
        }
    }
}

impl<V, const N: usize> Page<V, N> {
    pub(crate) fn get(&self, slot: usize) -> &V {
        &self.0[slot]
    }

    pub(crate) fn get_mut(&mut self, slot: usize) -> &mut V {
        &mut self.0[slot]
    }

    pub(crate) fn slice_mut(&mut self) -> &mut [V] {
        &mut self.0[..]
    }

    /// Returns every entry, in slot order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &V> {
        self.0.iter()
    }
}
