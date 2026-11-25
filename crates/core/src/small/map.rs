//! A map that searches a few inline entries and indexes them by hash once it grows.

use core::hash::Hash;
use std::collections::hash_map::Entry;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

/// A map that keeps up to `N` entries inline and searches them in order, and indexes its entries by
/// hash once it holds more.
///
/// Iteration follows insertion order.
#[derive(Clone, Debug)]
pub(crate) struct SmallMap<K, V, const N: usize> {
    entries: SmallVec<[(K, V); N]>,
    /// The position of every entry by key, once there are more than `N`.
    index: Option<FxHashMap<K, u32>>,
}

/// A set of up to `N` keys kept inline, which indexes its keys by hash once it holds more.
pub(crate) type SmallSet<K, const N: usize> = SmallMap<K, (), N>;

impl<K, V, const N: usize> Default for SmallMap<K, V, N> {
    fn default() -> Self {
        Self {
            entries: SmallVec::new(),
            index: None,
        }
    }
}

impl<K: Copy + Eq + Hash, V, const N: usize> SmallMap<K, V, N> {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        let position = self.position(key)?;
        self.entries.get(position).map(|(_, value)| value)
    }

    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.position(key).is_some()
    }

    /// Returns the value of `key`, inserting the value `make` returns if the map holds none.
    pub(crate) fn get_or_insert_with(&mut self, key: K, make: impl FnOnce() -> V) -> &mut V {
        let position = match self.slot(key) {
            Ok(position) => position,
            Err(position) => {
                self.entries.push((key, make()));
                self.pushed();
                position
            }
        };
        &mut self.entries[position].1
    }

    /// Inserts `value` at `key` and returns the value the key held.
    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.slot(key) {
            Ok(position) => Some(core::mem::replace(&mut self.entries[position].1, value)),
            Err(_) => {
                self.entries.push((key, value));
                self.pushed();
                None
            }
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, value)| value)
    }

    /// Returns the position of the entry of `key`.
    fn position(&self, key: &K) -> Option<usize> {
        match &self.index {
            Some(index) => index.get(key).map(|&position| position as usize),
            None => self.entries.iter().position(|(held, _)| held == key),
        }
    }

    /// Returns the position of the entry of `key`, or the position its entry takes when it is
    /// appended next, which the index then already holds.
    fn slot(&mut self, key: K) -> Result<usize, usize> {
        let next = self.entries.len();
        match &mut self.index {
            Some(index) => match index.entry(key) {
                Entry::Occupied(entry) => Ok(*entry.get() as usize),
                Entry::Vacant(entry) => {
                    entry.insert(next as u32);
                    Err(next)
                }
            },
            None => self
                .entries
                .iter()
                .position(|(held, _)| *held == key)
                .ok_or(next),
        }
    }

    /// Indexes the entries once an append made them more than `N`.
    fn pushed(&mut self) {
        if self.index.is_none() && self.entries.len() > N {
            self.index_all();
        }
    }

    /// Indexes every entry by its key.
    #[cold]
    #[inline(never)]
    fn index_all(&mut self) {
        let index = self
            .entries
            .iter()
            .enumerate()
            .map(|(position, (key, _))| (*key, position as u32))
            .collect();
        self.index = Some(index);
    }
}

impl<K: Copy + Eq + Hash, const N: usize> SmallSet<K, N> {
    /// Adds `key` and reports whether the set did not hold it.
    pub(crate) fn add(&mut self, key: K) -> bool {
        if self.slot(key).is_ok() {
            return false;
        }
        self.entries.push((key, ()));
        self.pushed();
        true
    }

    pub(crate) fn contains(&self, key: &K) -> bool {
        self.contains_key(key)
    }
}

impl<K: Copy + Eq + Hash, V, const N: usize> FromIterator<(K, V)> for SmallMap<K, V, N> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::default();
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}
