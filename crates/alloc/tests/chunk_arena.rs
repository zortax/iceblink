//! Storage, resolution, removal and recycling in a chunk arena.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::arena::Tally;
use iceblink_alloc::arena::{BLOCK_LEN, ChunkArena, Generation, Key, NamespaceId};

#[test]
fn a_value_is_readable_through_its_key() {
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let key = arena.insert(42_u32);
    assert_eq!(arena.get(key), Some(&42));
    assert!(arena.contains_key(key));
    assert_eq!(arena.len(), 1);
    assert!(!arena.is_empty());

    *arena.get_mut(key).expect("live") += 1;
    assert_eq!(arena.iter().collect::<Vec<_>>(), vec![&43]);
}

#[test]
fn addresses_hold_through_growth_and_removal_until_the_recycle() {
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let keys: Vec<Key<u64>> = (0..BLOCK_LEN as u64 * 4).map(|n| arena.insert(n)).collect();
    let address = |arena: &ChunkArena<u64>, key| arena.get(key).map(|value| value as *const u64);
    let before: Vec<_> = keys.iter().map(|key| address(&arena, *key)).collect();

    for n in 0..BLOCK_LEN as u64 * 4 {
        arena.insert(n);
    }
    for key in keys.iter().step_by(2) {
        assert!(arena.remove(*key));
    }

    let after: Vec<_> = keys.iter().map(|key| address(&arena, *key)).collect();
    assert_eq!(before, after);
    for (value, key) in keys.iter().enumerate() {
        assert_eq!(arena.get(*key), Some(&(value as u64)));
    }

    arena.recycle();
    for (value, key) in keys.iter().enumerate() {
        let expected = (value % 2 == 1).then_some(value as u64);
        assert_eq!(arena.get(*key).copied(), expected);
    }
}

#[test]
fn a_key_of_another_namespace_never_resolves() {
    let mut mine = ChunkArena::new(NamespaceId::FIRST);
    let mut theirs = ChunkArena::new(NamespaceId::new(1));
    let mine_key = mine.insert(1_u32);
    let theirs_key = theirs.insert(2_u32);
    assert_eq!(mine_key.index(), theirs_key.index());
    assert_eq!(mine_key.generation(), theirs_key.generation());

    assert_eq!(mine.get(theirs_key), None);
    assert_eq!(theirs.get(mine_key), None);
    assert!(!mine.remove(theirs_key));
    assert_eq!(mine.take(theirs_key), None);
}

#[test]
fn a_key_that_was_never_issued_does_not_resolve() {
    let arena: ChunkArena<u32> = ChunkArena::new(NamespaceId::FIRST);
    let invented = Key::new(0, Generation::FIRST, NamespaceId::FIRST);
    assert_eq!(arena.get(invented), None);
}

#[test]
fn a_hand_built_key_never_reaches_a_vacant_slot() {
    // A recycled slot carries the generation of its next occupant, so that generation is guessable.
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let key = arena.insert("stored".to_owned());
    arena.remove(key);
    arena.recycle();

    for raw in 1..=8_u16 {
        let guess = Key::new(
            0,
            Generation::new(raw).expect("non-zero"),
            NamespaceId::FIRST,
        );
        assert_eq!(arena.get(guess), None, "generation {raw}");
        assert!(!arena.remove(guess), "generation {raw}");
        assert_eq!(arena.take(guess), None, "generation {raw}");
    }
}

#[test]
fn a_removed_value_stays_readable_and_alive_until_the_recycle() {
    let drops = Rc::new(Cell::new(0));
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let key = arena.insert(Tally(Rc::clone(&drops)));

    assert!(arena.remove(key));
    assert!(!arena.remove(key));
    assert_eq!(arena.len(), 0);
    assert_eq!(drops.get(), 0);
    assert!(arena.get(key).is_some());
    assert_eq!(arena.iter().count(), 1);

    arena.recycle();
    assert_eq!(drops.get(), 1);
    assert!(arena.get(key).is_none());
}

#[test]
fn a_slot_is_reused_only_after_a_recycle_and_at_the_next_generation() {
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let first = arena.insert(1_u32);
    arena.remove(first);
    let second = arena.insert(2_u32);
    assert_ne!(second.index(), first.index());

    arena.recycle();
    let third = arena.insert(3_u32);
    assert_eq!(third.index(), first.index());
    assert_eq!(third.generation(), Generation::new(2).expect("non-zero"));
    assert_eq!(arena.get(first), None);
    assert_eq!(arena.get(third), Some(&3));
}

#[test]
fn take_returns_the_value_and_withholds_the_slot_until_the_recycle() {
    let drops = Rc::new(Cell::new(0));
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let key = arena.insert(Tally(Rc::clone(&drops)));

    let value = arena.take(key).expect("live");
    assert!(arena.get(key).is_none());
    assert!(arena.take(key).is_none());
    assert_eq!(arena.len(), 0);
    assert_eq!(arena.insert(Tally(Rc::clone(&drops))).index(), 1);

    arena.recycle();
    assert_eq!(drops.get(), 0);
    drop(value);
    assert_eq!(drops.get(), 1);

    let reused = arena.insert(Tally(Rc::clone(&drops)));
    assert_eq!(reused.index(), 0);
    assert_eq!(reused.generation(), Generation::new(2).expect("non-zero"));
}

#[test]
fn recycling_without_removals_changes_nothing() {
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let key = arena.insert(1_u32);
    arena.recycle();
    arena.recycle();
    assert_eq!(arena.get(key), Some(&1));
    assert_eq!(arena.len(), 1);
}

#[test]
fn a_value_can_be_built_from_its_own_key() {
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let first = arena.insert_with(|key| key.index());
    arena.remove(first);
    arena.recycle();

    let second = arena.insert_with(|key| key.index());
    assert_eq!(second.index(), first.index());
    assert_eq!(arena.get(second), Some(&second.index()));
}

#[test]
fn a_panicking_build_stores_nothing() {
    let mut arena: ChunkArena<String> = ChunkArena::new(NamespaceId::FIRST);
    let kept = arena.insert("kept".to_owned());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        arena.insert_with(|_| panic!("the value could not be built"))
    }));
    assert!(outcome.is_err());

    // Reading and then dropping the arena must not touch the unwritten slot.
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.iter().count(), 1);
    assert_eq!(arena.get(kept).map(String::as_str), Some("kept"));
}

#[test]
fn dropping_the_arena_drops_live_and_removed_values_once() {
    let drops = Rc::new(Cell::new(0));
    {
        let mut arena = ChunkArena::new(NamespaceId::FIRST);
        for _ in 0..3 {
            arena.insert(Tally(Rc::clone(&drops)));
        }

        let recycled = arena.insert(Tally(Rc::clone(&drops)));
        arena.remove(recycled);
        arena.recycle();
        assert_eq!(drops.get(), 1);

        let removed = arena.insert(Tally(Rc::clone(&drops)));
        arena.remove(removed);
        assert_eq!(drops.get(), 1);
    }
    assert_eq!(drops.get(), 5);
}

#[test]
fn zero_sized_values_are_stored() {
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let keys: Vec<Key<()>> = (0..BLOCK_LEN + 1).map(|_| arena.insert(())).collect();
    assert!(keys.iter().all(|key| arena.get(*key) == Some(&())));
    assert_eq!(arena.take(keys[0]), Some(()));
}

#[test]
fn the_debug_form_reports_counts_without_values() {
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    arena.insert(1_u32);
    let text = format!("{arena:?}");
    assert!(text.contains("len: 1"), "{text}");
    assert!(text.contains("retired: 0"), "{text}");
}

#[test]
fn an_arena_is_send_and_sync_when_its_values_are() {
    const fn assert_sync<T: Sync>() {}
    const fn assert_send<T: Send>() {}
    assert_sync::<ChunkArena<u32>>();
    assert_send::<ChunkArena<u32>>();
}
