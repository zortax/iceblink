//! The dense side table and its entries.

mod common;

use std::collections::BTreeMap;

use common::arena::key;
use iceblink_alloc::arena::{Key, NamespaceId, SlotVec};
use proptest::prelude::*;

#[test]
fn a_slot_past_the_end_reads_as_absent() {
    let table: SlotVec<Key<()>, u32> = SlotVec::new();
    assert_eq!(table.get(key(0)), None);
    assert_eq!(table.get(key(1_000_000)), None);
    assert!(table.is_empty());
}

#[test]
fn writing_a_high_slot_leaves_lower_slots_empty() {
    let mut table: SlotVec<Key<()>, u32> = SlotVec::new();
    table.insert(key(4), 9);
    assert_eq!(table.len(), 1);
    assert_eq!(table.get(key(0)), None);
    assert_eq!(table.get(key(4)), Some(&9));
    assert_eq!(table.iter().collect::<Vec<_>>(), vec![(4, &9)]);
}

#[test]
fn modifications_through_borrows_are_kept() {
    let mut table: SlotVec<Key<()>, u32> = SlotVec::new();
    table.insert(key(2), 1);
    *table.get_mut(key(2)).expect("stored") += 1;
    for (_, value) in table.iter_mut() {
        *value += 1;
    }
    assert_eq!(table.get(key(2)), Some(&3));
    assert!(table.contains_key(key(2)));

    table.clear();
    assert!(table.is_empty());
    assert_eq!(table.get(key(2)), None);
}

#[test]
fn an_entry_fills_an_empty_slot_once() {
    let mut table: SlotVec<Key<()>, u32> = SlotVec::new();
    assert!(!table.entry(key(0)).is_occupied());
    assert_eq!(*table.entry(key(0)).or_insert(1), 1);
    assert_eq!(*table.entry(key(0)).or_insert(2), 1);
    assert!(table.entry(key(0)).is_occupied());
    assert_eq!(table.len(), 1);
}

#[test]
fn and_modify_changes_only_an_occupied_slot() {
    let mut table: SlotVec<Key<()>, u32> = SlotVec::new();
    table.entry(key(0)).and_modify(|value| *value += 1);
    assert_eq!(table.get(key(0)), None);

    table.insert(key(0), 1);
    table.entry(key(0)).and_modify(|value| *value += 1);
    assert_eq!(table.get(key(0)), Some(&2));
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "a key from another namespace")]
fn a_table_for_one_namespace_rejects_a_key_of_another() {
    let mut table: SlotVec<Key<()>, u32> = SlotVec::for_namespace(NamespaceId::new(1));
    table.insert(key(0), 1);
}

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn the_table_behaves_like_a_map_keyed_by_slot(
        operations in proptest::collection::vec((0_u32..32, any::<Option<u32>>()), 0..200),
    ) {
        let mut table: SlotVec<Key<()>, u32> = SlotVec::new();
        let mut oracle: BTreeMap<u32, u32> = BTreeMap::new();

        for (index, operation) in operations {
            match operation {
                Some(value) => {
                    prop_assert_eq!(table.insert(key(index), value), oracle.insert(index, value));
                }
                None => {
                    prop_assert_eq!(table.remove(key(index)), oracle.remove(&index));
                }
            }
            prop_assert_eq!(table.len(), oracle.len());
        }

        let stored: Vec<(u32, u32)> = table.iter().map(|(index, value)| (index, *value)).collect();
        let expected: Vec<(u32, u32)> = oracle.into_iter().collect();
        prop_assert_eq!(stored, expected);
    }
}
