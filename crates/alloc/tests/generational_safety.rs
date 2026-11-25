//! A key resolves to its own value or to nothing, checked against a model of the arena.

mod common;

use std::collections::HashSet;

use iceblink_alloc::arena::{ChunkArena, Generation, Key, NamespaceId};
use proptest::prelude::*;

/// An operation applied to the arena.
#[derive(Copy, Clone, Debug)]
enum Op {
    /// Inserts a value.
    Insert(u32),
    /// Removes the value of the nth live key.
    Remove(usize),
    /// Takes the value of the nth resolving key.
    Take(usize),
    /// Recycles removed values.
    Recycle,
}

/// The expected answers of the arena.
#[derive(Default)]
struct Model {
    /// Keys that resolve, with their value and whether the value is live.
    resolving: Vec<(Key<u32>, u32, bool)>,
    /// Keys that never resolve again.
    dead: Vec<Key<u32>>,
}

impl Model {
    /// Checks that the arena agrees with the model.
    fn check(&self, arena: &ChunkArena<u32>) -> Result<(), TestCaseError> {
        for (key, value, _) in &self.resolving {
            prop_assert_eq!(
                arena.get(*key),
                Some(value),
                "a key that was issued and not yet dropped must resolve to its own value"
            );
        }
        for key in &self.dead {
            prop_assert_eq!(
                arena.get(*key),
                None,
                "a key whose value is gone must never resolve, however its slot is reused"
            );
        }
        let live = self.resolving.iter().filter(|(_, _, live)| *live).count();
        prop_assert_eq!(arena.len() as usize, live);
        Ok(())
    }

    /// Returns every key the arena has issued.
    fn issued(&self) -> impl Iterator<Item = Key<u32>> {
        self.resolving
            .iter()
            .map(|(key, _, _)| *key)
            .chain(self.dead.iter().copied())
    }
}

fn run(ops: Vec<Op>) -> Result<(), TestCaseError> {
    let mut arena: ChunkArena<u32> = ChunkArena::new(NamespaceId::FIRST);
    let mut model = Model::default();

    for op in ops {
        match op {
            Op::Insert(value) => {
                let key = arena.insert(value);
                let seen: HashSet<Key<u32>> = model.issued().collect();
                prop_assert!(
                    !seen.contains(&key),
                    "a key is issued at most once for the whole life of the arena"
                );
                model.resolving.push((key, value, true));
            }
            Op::Remove(nth) => {
                if let Some(entry) = pick(&mut model.resolving, nth, |(_, _, live)| *live) {
                    prop_assert!(arena.remove(entry.0));
                    prop_assert!(!arena.remove(entry.0), "removing twice changes nothing");
                    entry.2 = false;
                }
            }
            Op::Take(nth) => {
                let taken = pick(&mut model.resolving, nth, |_| true).map(|entry| *entry);
                if let Some((key, value, _)) = taken {
                    prop_assert_eq!(arena.take(key), Some(value));
                    model.resolving.retain(|(other, _, _)| *other != key);
                    model.dead.push(key);
                }
            }
            Op::Recycle => {
                arena.recycle();
                let Model { resolving, dead } = &mut model;
                resolving.retain(|(key, _, live)| {
                    if *live {
                        true
                    } else {
                        dead.push(*key);
                        false
                    }
                });
            }
        }
        model.check(&arena)?;
    }
    Ok(())
}

/// Returns the nth entry that matches, counting modulo the number of matches.
fn pick<T>(entries: &mut [T], nth: usize, matches: impl Fn(&T) -> bool) -> Option<&mut T> {
    let positions: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| matches(entry))
        .map(|(index, _)| index)
        .collect();
    let position = *positions.get(nth % positions.len().max(1))?;
    entries.get_mut(position)
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => any::<u32>().prop_map(Op::Insert),
        4 => any::<usize>().prop_map(Op::Remove),
        1 => any::<usize>().prop_map(Op::Take),
        3 => Just(Op::Recycle),
    ]
}

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn a_key_resolves_to_its_own_value_or_to_nothing(ops in proptest::collection::vec(op(), 0..200)) {
        run(ops)?;
    }
}

#[test]
fn a_slot_that_runs_out_of_counters_is_retired_and_its_keys_stay_dead() {
    let mut arena: ChunkArena<u32> = ChunkArena::new(NamespaceId::FIRST);
    let mut issued: Vec<Key<u32>> = Vec::new();

    // Slot 0 is reused until its generation runs out.
    while arena.retired() == 0 {
        let key = arena.insert(issued.len() as u32);
        assert_eq!(key.index(), 0, "the same slot comes back round every frame");
        assert_eq!(
            usize::from(key.generation().get()),
            issued.len() + 1,
            "each occupant of a slot gets the next counter, and no counter repeats"
        );
        issued.push(key);
        arena.remove(key);
        arena.recycle();
    }

    assert_eq!(issued.len(), usize::from(u16::MAX));
    assert_eq!(issued[0].index(), 0);
    assert_eq!(issued[0].generation(), Generation::FIRST);
    assert_eq!(issued[issued.len() - 1].generation(), Generation::LAST);

    let next = arena.insert(0);
    assert_eq!(
        next.index(),
        1,
        "the retired slot is never handed out again"
    );
    assert_eq!(next.generation(), Generation::FIRST);

    for key in &issued {
        assert_eq!(arena.get(*key), None, "no key from a retired slot resolves");
    }
}
