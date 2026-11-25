//! The order nodes whose sequences hold each order entry, once per entry.
//!
//! An entry that one sequence holds once is stored as its parent alone, and the parents of an
//! entry held more than once are kept apart.

mod chunks;
mod nodes;

pub(crate) use self::chunks::ChunkParents;
pub(crate) use self::nodes::NodeParents;

use core::hash::Hash;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

/// The stored parent of an entry held more than once.
const MANY: u32 = u32::MAX - 1;

/// The parents of the entries held more than once.
#[derive(Debug)]
struct Many<K>(FxHashMap<K, SmallVec<[u32; 2]>>);

impl<K> Default for Many<K> {
    fn default() -> Self {
        Self(FxHashMap::default())
    }
}

impl<K: Hash + Eq> Many<K> {
    /// Returns the parents of `key`, or none when it is held at most once.
    fn get(&self, key: &K) -> &[u32] {
        self.0.get(key).map_or(&[], |parents| parents.as_slice())
    }

    /// Adds `parent` to the parents of `key`, whose stored parent so far is `held`.
    fn add(&mut self, key: K, held: u32, parent: u32) {
        if held == MANY {
            self.0.entry(key).or_default().push(parent);
        } else {
            self.0.insert(key, SmallVec::from_slice(&[held, parent]));
        }
    }

    /// Removes `parent` once from the parents of `key`, and returns the one parent left, `None`
    /// when none is left, or `MANY`.
    fn remove(&mut self, key: &K, parent: u32) -> Option<u32> {
        let parents = self.0.get_mut(key)?;
        if let Some(index) = parents.iter().position(|&held| held == parent) {
            parents.swap_remove(index);
        }

        let left = match parents.as_slice() {
            [] => None,
            &[last] => Some(last),
            _ => return Some(MANY),
        };

        self.0.remove(key);
        left
    }
}
