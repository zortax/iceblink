//! The sparse side table.

mod compact;
mod disjoint;
mod page;
mod write;
mod written;

use core::marker::PhantomData;

use crate::arena::key::{ArenaKey, NamespaceId};
use crate::arena::paged_vec::page::Page;
use crate::arena::paged_vec::written::Written;

pub use crate::arena::paged_vec::page::PAGE_LEN;

/// A sparse side table that stores values in pages of `N` slots, each allocated on first write.
///
/// Reading a slot on an unallocated page returns `None` and allocates nothing. Writing a slot
/// allocates its page with every entry at the default. [`PagedVec::compact`] drops the pages whose
/// entries are all default again.
///
/// A table created with [`PagedVec::for_namespace`] panics in debug builds when given a key of
/// another namespace.
///
/// ```
/// use iceblink_alloc::arena::{ChunkArena, NamespaceId, PagedVec};
///
/// let mut arena: ChunkArena<&str> = ChunkArena::new(NamespaceId::FIRST);
/// let key = arena.insert("item");
///
/// let mut labels: PagedVec<_, Option<String>> = PagedVec::for_namespace(arena.namespace());
/// assert_eq!(labels.get(key), None);
///
/// *labels.get_mut(key) = Some("hello".to_owned());
/// assert_eq!(labels.get(key), Some(&Some("hello".to_owned())));
/// assert_eq!(labels.pages(), 1);
///
/// labels.clear(key);
/// labels.compact();
/// assert_eq!(labels.pages(), 0);
/// ```
pub struct PagedVec<K: ArenaKey, V, const N: usize = PAGE_LEN> {
    /// The page index, `None` where a page is not allocated.
    pages: Vec<Option<Page<V, N>>>,
    /// The pages written since the last compaction.
    written: Written,
    /// The namespace keys are checked against, if any.
    namespace: Option<NamespaceId>,
    key: PhantomData<fn() -> K>,
}

impl<K: ArenaKey, V, const N: usize> PagedVec<K, V, N> {
    /// Creates an empty table that accepts keys of every namespace.
    pub const fn new() -> Self {
        Self {
            pages: Vec::new(),
            written: Written::new(),
            namespace: None,
            key: PhantomData,
        }
    }

    /// Creates an empty table for the keys of `namespace`.
    pub const fn for_namespace(namespace: NamespaceId) -> Self {
        Self {
            pages: Vec::new(),
            written: Written::new(),
            namespace: Some(namespace),
            key: PhantomData,
        }
    }

    /// Returns the number of allocated pages.
    pub fn pages(&self) -> usize {
        self.pages.iter().flatten().count()
    }

    /// Returns the value stored for `key`, or `None` if its page is not allocated.
    pub fn get(&self, key: K) -> Option<&V> {
        self.check(key);
        let (page, slot) = split::<K, N>(key);
        Some(self.pages.get(page)?.as_ref()?.get(slot))
    }

    /// Returns every entry of every allocated page with its slot index, in slot order.
    ///
    /// Entries that hold the default are included.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &V)> {
        self.pages
            .iter()
            .enumerate()
            .filter_map(|(index, page)| Some((index, page.as_ref()?)))
            .flat_map(|(index, page)| {
                page.iter()
                    .enumerate()
                    .map(move |(slot, value)| ((index * N + slot) as u32, value))
            })
    }

    /// Panics in debug builds if `key` belongs to another namespace.
    #[inline]
    fn check(&self, key: K) {
        debug_assert!(
            self.namespace
                .is_none_or(|namespace| namespace == key.namespace()),
            "a key from another namespace was used on a side table"
        );
    }
}

/// Splits a key into its page and its slot within the page.
fn split<K: ArenaKey, const N: usize>(key: K) -> (usize, usize) {
    assert!(N > 0, "a sparse-table page must hold at least one entry");
    let index = key.index() as usize;
    (index / N, index % N)
}

impl<K: ArenaKey, V, const N: usize> Default for PagedVec<K, V, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: ArenaKey, V, const N: usize> core::fmt::Debug for PagedVec<K, V, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PagedVec")
            .field("pages", &self.pages())
            .field("indexed", &self.pages.len())
            .finish()
    }
}
