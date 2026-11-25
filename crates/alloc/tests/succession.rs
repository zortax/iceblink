//! Handing a namespace on to a successor arena.

use iceblink_alloc::arena::{BLOCK_LEN, ChunkArena, Generation, Key, NamespaceId};

#[test]
fn no_key_of_the_old_arena_resolves_in_its_successor() {
    let mut old = ChunkArena::new(NamespaceId::FIRST);
    let live = old.insert(1_u32);
    let recycled = old.insert(2_u32);
    let taken = old.insert(3_u32);
    old.remove(recycled);
    old.recycle();
    let pending = old.insert(4_u32);
    old.remove(pending);
    old.take(taken);
    let succession = old.succession();
    drop(old);

    let mut new = ChunkArena::succeeding(NamespaceId::FIRST, &succession);
    let keys: Vec<Key<u32>> = (0..4).map(|n| new.insert(n)).collect();
    for stale in [live, recycled, taken, pending] {
        assert_eq!(new.get(stale), None, "{stale:?} resolves in the successor");
    }
    assert_eq!(keys[0].index(), 0, "the lowest slot comes first");
    assert!(keys.iter().all(|key| new.get(*key).is_some()));
}

#[test]
fn a_slot_at_its_last_generation_stays_retired() {
    let mut arena = ChunkArena::new(NamespaceId::FIRST);
    let mut key = arena.insert(0_u32);
    while key.generation() != Generation::LAST {
        arena.remove(key);
        arena.recycle();
        key = arena.insert(0);
    }
    let succession = arena.succession();
    assert_eq!(succession.next_generation(0), None);
    assert_eq!(succession.retired(), 1);

    let mut new: ChunkArena<u32> = ChunkArena::succeeding(NamespaceId::FIRST, &succession);
    assert_eq!(new.retired(), 1);
    assert_eq!(new.insert(1).index(), 1);
}

// Retires a whole block of slots, which takes hours under miri.
#[cfg_attr(miri, ignore)]
#[test]
fn a_successor_whose_first_free_slot_is_past_a_retired_block_stores_there() {
    let mut old = ChunkArena::new(NamespaceId::FIRST);
    let mut keys: Vec<Key<u32>> = (0..BLOCK_LEN as u32).map(|n| old.insert(n)).collect();
    while keys[0].generation() != Generation::LAST {
        for key in &keys {
            old.remove(*key);
        }
        old.recycle();
        keys = (0..BLOCK_LEN as u32).map(|n| old.insert(n)).collect();
    }
    let succession = old.succession();
    drop(old);
    assert_eq!(succession.retired() as usize, BLOCK_LEN);

    let mut new: ChunkArena<u32> = ChunkArena::succeeding(NamespaceId::FIRST, &succession);
    let key = new.insert(7);
    assert_eq!(key.index() as usize, BLOCK_LEN);
    assert_eq!(new.get(key), Some(&7));
}

#[test]
fn an_empty_succession_starts_a_fresh_arena() {
    let succession = ChunkArena::<u32>::new(NamespaceId::FIRST).succession();
    assert!(succession.is_empty());
    assert_eq!(succession.len(), 0);
    assert_eq!(succession.next_generation(7), Some(Generation::FIRST));

    let mut arena: ChunkArena<u32> = ChunkArena::succeeding(NamespaceId::FIRST, &succession);
    let key = arena.insert(1);
    assert_eq!((key.index(), key.generation()), (0, Generation::FIRST));
}
