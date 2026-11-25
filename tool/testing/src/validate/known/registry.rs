//! A renderer's set of keyed objects: pages, shaders or resident chunks.

use std::collections::BTreeMap;

use crate::validate::known::Lookup;

/// A renderer's keyed objects, as far as the frames seen so far tell them.
#[derive(Clone, Debug)]
pub(crate) struct Registry<K, T> {
    /// Whether every key without an entry is known to hold nothing.
    complete: bool,
    /// The keys seen, with `None` for a key known to hold nothing.
    entries: BTreeMap<K, Option<T>>,
}

impl<K: Ord + Copy, T> Registry<K, T> {
    /// Returns a registry known to hold nothing.
    pub(crate) fn empty() -> Self {
        Self {
            complete: true,
            entries: BTreeMap::new(),
        }
    }

    /// Returns a registry whose contents are unknown.
    pub(crate) fn unknown() -> Self {
        Self {
            complete: false,
            entries: BTreeMap::new(),
        }
    }

    /// Returns what is known about `key`.
    pub(crate) fn get(&self, key: K) -> Lookup<&T> {
        match self.entries.get(&key) {
            Some(Some(value)) => Lookup::Found(value),
            Some(None) => Lookup::Missing,
            None if self.complete => Lookup::Missing,
            None => Lookup::Unknown,
        }
    }

    /// Stores `value` at `key`, and returns what was known about the key before.
    pub(crate) fn insert(&mut self, key: K, value: T) -> Lookup<()> {
        let before = self.state(key);
        self.entries.insert(key, Some(value));
        before
    }

    /// Empties `key`, and returns what was known about the key before.
    pub(crate) fn remove(&mut self, key: K) -> Lookup<()> {
        let before = self.state(key);
        self.entries.insert(key, None);
        before
    }

    /// Forgets every object, leaving a registry known to hold nothing.
    pub(crate) fn clear(&mut self) {
        self.complete = true;
        self.entries.clear();
    }

    fn state(&self, key: K) -> Lookup<()> {
        match self.get(key) {
            Lookup::Found(_) => Lookup::Found(()),
            Lookup::Missing => Lookup::Missing,
            Lookup::Unknown => Lookup::Unknown,
        }
    }
}
