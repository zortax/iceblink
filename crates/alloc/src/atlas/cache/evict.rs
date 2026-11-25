//! Eviction by generation.

use crate::atlas::cache::Atlas;
use crate::atlas::key::AtlasKey;

/// The entries one eviction removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Eviction {
    /// The generation evicted last, or `None` if nothing was evicted.
    pub generation: Option<u64>,
    /// The number of entries removed.
    pub tiles: usize,
    /// The texels of tile space freed.
    pub texels: u64,
}

impl Eviction {
    /// Reports whether nothing was removed.
    pub const fn is_empty(&self) -> bool {
        self.tiles == 0
    }

    /// Adds the removals of `step`.
    fn add(&mut self, step: Self) {
        self.generation = step.generation;
        self.tiles += step.tiles;
        self.texels += step.texels;
    }
}

impl Atlas {
    /// Removes the evictable entries of the oldest generation.
    ///
    /// An entry is evictable if:
    ///
    /// 1. it has no holds, and
    /// 2. it is not used in the current generation.
    ///
    /// Returns an empty eviction if no entry is evictable.
    pub fn evict_least_recently_used(&mut self) -> Eviction {
        self.evict_least_recently_used_into(&mut Vec::new())
    }

    /// Removes the evictable entries of the oldest generation, as
    /// [`Atlas::evict_least_recently_used`] does, and appends their keys to `removed`.
    pub fn evict_least_recently_used_into(&mut self, removed: &mut Vec<AtlasKey>) -> Eviction {
        let Some(generation) = self.evictable().map(|(_, age, _)| age).min() else {
            return Eviction::default();
        };
        let doomed: Vec<(AtlasKey, u64)> = self
            .evictable()
            .filter(|(_, age, _)| *age == generation)
            .map(|(key, _, texels)| (key, texels))
            .collect();

        let mut eviction = Eviction {
            generation: Some(generation),
            tiles: 0,
            texels: 0,
        };
        for (key, texels) in doomed {
            if self.remove(key) {
                removed.push(key);
                eviction.tiles += 1;
                eviction.texels += texels;
            }
        }
        eviction
    }

    /// Removes every evictable entry, oldest generation first, and returns the total.
    pub fn evict_all_unused(&mut self) -> Eviction {
        let mut total = Eviction::default();
        loop {
            let step = self.evict_least_recently_used();
            if step.is_empty() {
                return total;
            }
            total.add(step);
        }
    }

    /// Removes evictable entries, oldest generation first, until [`Atlas::resident_bytes`] is at
    /// most [`AtlasLimits::soft_bytes`](crate::atlas::AtlasLimits::soft_bytes).
    ///
    /// Nothing is removed if no soft limit is set. Eviction stops when no evictable entry is left,
    /// so the atlas can stay above the limit. Resident bytes fall only when a texture loses its
    /// last tile, so a generation can be evicted without freeing any bytes.
    pub fn evict_to_soft_limit(&mut self) -> Eviction {
        self.evict_to_soft_limit_into(&mut Vec::new())
    }

    /// Removes evictable entries until the soft limit is met, as [`Atlas::evict_to_soft_limit`]
    /// does, and appends their keys to `removed`.
    pub fn evict_to_soft_limit_into(&mut self, removed: &mut Vec<AtlasKey>) -> Eviction {
        let Some(soft) = self.limits.soft_bytes else {
            return Eviction::default();
        };
        let mut total = Eviction::default();
        while self.resident_bytes() > soft {
            let step = self.evict_least_recently_used_into(removed);
            if step.is_empty() {
                break;
            }
            total.add(step);
        }
        total
    }

    /// Returns the key, generation and texels of every evictable entry.
    fn evictable(&self) -> impl Iterator<Item = (AtlasKey, u64, u64)> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(slot, held)| held.as_ref().map(|entry| (slot, entry)))
            .filter(|(slot, entry)| entry.is_unreferenced() && !self.used.contains(*slot))
            .map(|(_, entry)| (entry.key, entry.generation, entry.texels()))
    }
}
