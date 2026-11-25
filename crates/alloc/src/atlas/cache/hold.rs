//! Holds that protect entries from eviction.

use crate::atlas::cache::Atlas;
use crate::atlas::key::AtlasKey;

impl Atlas {
    /// Holds `key` against eviction, and returns whether `key` is cached.
    ///
    /// The hold count saturates at `u32::MAX`.
    pub fn retain(&mut self, key: AtlasKey) -> bool {
        let Some(&slot) = self.index.get(&key) else {
            return false;
        };
        if let Some(entry) = self.entries[slot as usize].as_mut() {
            if entry.is_unreferenced() {
                self.held_tiles += 1;
                self.held_bytes += entry.bytes();
            }
            entry.refs = entry.refs.saturating_add(1);
        }
        true
    }

    /// Releases one hold on `key`, and returns whether `key` is cached.
    ///
    /// Releasing an entry without holds does nothing.
    pub fn release(&mut self, key: AtlasKey) -> bool {
        let Some(&slot) = self.index.get(&key) else {
            return false;
        };
        if let Some(entry) = self.entries[slot as usize].as_mut()
            && !entry.is_unreferenced()
        {
            entry.refs -= 1;
            if entry.is_unreferenced() {
                self.held_tiles -= 1;
                self.held_bytes -= entry.bytes();
            }
        }
        true
    }

    /// Returns the hold count of `key`, or `None` if `key` is not cached.
    pub fn refs(&self, key: AtlasKey) -> Option<u32> {
        let slot = *self.index.get(&key)?;
        self.entries[slot as usize].map(|entry| entry.refs)
    }
}
