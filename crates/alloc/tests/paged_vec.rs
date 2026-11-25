//! The sparse side table: page allocation, compaction and disjoint borrows.

mod common;

use std::cell::Cell;
use std::collections::BTreeMap;

use common::arena::key;
use iceblink_alloc::arena::{ChunkArena, Key, NamespaceId, PAGE_LEN, PagedVec};
use proptest::prelude::*;

const PAGE: u32 = PAGE_LEN as u32;

#[test]
fn reading_or_clearing_an_unallocated_page_allocates_nothing() {
    let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
    assert_eq!(table.get(key(0)), None);
    assert_eq!(table.get(key(u32::MAX)), None);
    table.clear(key(500));
    assert_eq!(table.pages(), 0);
}

#[test]
fn a_write_allocates_exactly_its_page() {
    let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
    *table.get_mut(key(3 * PAGE)) = 7;
    assert_eq!(table.pages(), 1);
    assert_eq!(table.get(key(3 * PAGE)), Some(&7));
    assert_eq!(table.get(key(3 * PAGE + 1)), Some(&0));
    assert_eq!(table.get(key(0)), None);
}

#[test]
fn replace_returns_the_previous_value() {
    let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
    assert_eq!(table.replace(key(1), 5), 0);
    assert_eq!(table.replace(key(1), 6), 5);
    table.clear(key(1));
    assert_eq!(table.get(key(1)), Some(&0));
}

#[test]
fn iteration_covers_allocated_pages_in_slot_order() {
    let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
    *table.get_mut(key(PAGE)) = 3;
    assert_eq!(table.iter().count(), PAGE_LEN);
    let stored: Vec<(u32, u32)> = table
        .iter()
        .filter(|(_, value)| **value != 0)
        .map(|(index, value)| (index, *value))
        .collect();
    assert_eq!(stored, vec![(PAGE, 3)]);
}

#[test]
fn compaction_drops_pages_emptied_by_clear_or_by_writing_the_default() {
    let mut table: PagedVec<Key<()>, Option<u32>> = PagedVec::new();
    *table.get_mut(key(1)) = Some(1);
    *table.get_mut(key(PAGE + 1)) = Some(2);
    *table.get_mut(key(2 * PAGE + 1)) = Some(3);
    table.compact();
    assert_eq!(table.pages(), 3);

    table.clear(key(1));
    *table.get_mut(key(PAGE + 1)) = None;
    table.compact();
    assert_eq!(table.pages(), 1);
    assert_eq!(table.get(key(1)), None);
    assert_eq!(table.get(key(2 * PAGE + 1)), Some(&Some(3)));
}

#[test]
fn compaction_tests_only_pages_written_since_the_last_one() {
    let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
    for page in 0..16 {
        *table.get_mut(key(page * PAGE + 3)) = 9;
    }
    let examined = Cell::new(0_usize);
    let count = |value: &u32| {
        examined.set(examined.get() + 1);
        *value == 0
    };
    table.compact_by(count);
    assert!(examined.get() > 0);

    examined.set(0);
    table.compact_by(count);
    assert_eq!(examined.get(), 0);
    assert_eq!(table.pages(), 16);
}

#[test]
fn a_page_emptied_after_several_compactions_is_still_dropped() {
    let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
    *table.get_mut(key(2)) = 5;
    for _ in 0..4 {
        table.compact();
    }
    assert_eq!(table.pages(), 1);
    table.clear(key(2));
    table.compact();
    assert_eq!(table.pages(), 0);
}

/// A value without `PartialEq`.
#[derive(Default)]
struct Uncomparable(Vec<u32>);

#[test]
fn compact_by_uses_the_given_default_test() {
    let mut table: PagedVec<Key<()>, Uncomparable> = PagedVec::new();
    table.get_mut(key(0)).0.push(1);
    table.get_mut(key(PAGE)).0.push(2);
    table.compact_by(|entry| entry.0.is_empty());
    assert_eq!(table.pages(), 2);

    table.get_mut(key(PAGE)).0.clear();
    table.compact_by(|entry| entry.0.is_empty());
    assert_eq!(table.pages(), 1);
    assert_eq!(
        table.get(key(0)).map(|entry| entry.0.as_slice()),
        Some(&[1][..])
    );
    assert!(table.get(key(PAGE)).is_none());
}

#[test]
fn disjoint_borrows_span_pages_and_write_independently() {
    let mut arena: ChunkArena<u32> = ChunkArena::new(NamespaceId::FIRST);
    let keys: Vec<_> = (0..2100).map(|value| arena.insert(value)).collect();
    let mut table: PagedVec<_, u32> = PagedVec::for_namespace(arena.namespace());
    for &key in &keys {
        *table.get_mut(key) = 1;
    }

    let picked = [keys[0], keys[1], keys[1023], keys[1024], keys[2099]];
    let borrows = table.disjoint_mut(&picked);
    assert_eq!(borrows.len(), 5);
    for (offset, entry) in borrows.into_iter().enumerate() {
        *entry += offset as u32;
    }
    assert_eq!(table.get(picked[0]), Some(&1));
    assert_eq!(table.get(picked[3]), Some(&4));
    assert_eq!(table.get(picked[4]), Some(&5));
}

#[test]
#[should_panic(expected = "strictly ascending")]
fn disjoint_borrows_refuse_unsorted_keys() {
    let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
    *table.get_mut(key(0)) = 1;
    *table.get_mut(key(1)) = 1;
    let _ = table.disjoint_mut(&[key(1), key(0)]);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "a key from another namespace")]
fn a_table_for_one_namespace_rejects_a_key_of_another() {
    let table: PagedVec<Key<()>, u32> = PagedVec::for_namespace(NamespaceId::new(1));
    let _ = table.get(key(0));
}

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn a_scattered_table_costs_one_page_per_written_page(
        pages in proptest::collection::hash_set(0_u32..64, 0..24),
    ) {
        let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
        let slots: Vec<u32> = pages.iter().map(|page| page * PAGE + 1).collect();
        for slot in &slots {
            *table.get_mut(key(*slot)) = 1;
        }
        prop_assert_eq!(table.pages(), pages.len());

        for slot in &slots {
            table.clear(key(*slot));
        }
        table.compact();
        prop_assert_eq!(table.pages(), 0);
    }

    #[test]
    fn compaction_never_loses_a_value(
        writes in proptest::collection::vec((0_u32..4096, 1_u32..8), 0..64),
    ) {
        let mut table: PagedVec<Key<()>, u32> = PagedVec::new();
        let mut expected = BTreeMap::new();
        for (slot, value) in writes {
            *table.get_mut(key(slot)) = value;
            expected.insert(slot, value);
        }
        table.compact();
        for (slot, value) in expected {
            prop_assert_eq!(table.get(key(slot)), Some(&value));
        }
    }
}
