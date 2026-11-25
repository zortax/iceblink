//! The order nodes whose sequences hold each chunk key.

use std::collections::hash_map::Entry;

use iceblink::ChunkKey;
use rustc_hash::FxHashMap;

use crate::scene::parents::{MANY, Many};

/// The order nodes whose sequences hold each chunk key, once per entry.
#[derive(Debug, Default)]
pub(crate) struct ChunkParents {
    /// The parent of each key a sequence holds, or `MANY`.
    one: FxHashMap<ChunkKey, u32>,
    many: Many<ChunkKey>,
}

impl ChunkParents {
    /// Returns the nodes whose sequences hold `key`, once per entry.
    #[inline]
    pub(crate) fn get(&self, key: &ChunkKey) -> &[u32] {
        match self.one.get(key) {
            None => &[],
            Some(&MANY) => self.many.get(key),
            Some(parent) => core::slice::from_ref(parent),
        }
    }

    /// Returns the first node whose sequence holds `key`.
    #[inline]
    pub(crate) fn first(&self, key: &ChunkKey) -> Option<u32> {
        self.get(key).first().copied()
    }

    /// Notes that the sequence of `parent` holds `key` once more, and reports whether sequences
    /// now hold it more than once.
    pub(crate) fn place(&mut self, key: ChunkKey, parent: u32) -> bool {
        match self.one.entry(key) {
            Entry::Vacant(vacant) => {
                vacant.insert(parent);
                false
            }
            Entry::Occupied(mut held) => {
                self.many.add(key, *held.get(), parent);
                held.insert(MANY);
                true
            }
        }
    }

    /// Notes for every key and node of `held` that the node's sequence holds the key once less.
    pub(crate) fn unplace_all(&mut self, held: &[(ChunkKey, u32)]) {
        // Looking up a key a few pairs ahead overlaps the cache misses of the large map.
        const AHEAD: usize = 8;
        for (index, &(key, parent)) in held.iter().enumerate() {
            if let Some((ahead, _)) = held.get(index + AHEAD) {
                core::hint::black_box(self.one.get(ahead));
            }
            self.unplace(key, parent);
        }
    }

    /// Notes that the sequence of `parent` holds `key` once less.
    pub(crate) fn unplace(&mut self, key: ChunkKey, parent: u32) {
        let Entry::Occupied(mut held) = self.one.entry(key) else {
            return;
        };

        let left = match *held.get() {
            MANY => self.many.remove(&key, parent),
            one if one == parent => None,
            _ => return,
        };

        match left {
            Some(left) => {
                held.insert(left);
            }
            None => {
                held.remove();
            }
        }
    }
}
