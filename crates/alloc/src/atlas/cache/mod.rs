//! The atlas: cached entries, holds, generations and the upload queue.

pub mod evict;
pub mod limits;
pub mod report;

mod entry;
mod flush;
mod hold;
mod insert;
mod pool;
mod remove;
mod upload;

use iceblink::damage::EpochBitset;
use rustc_hash::FxHashMap;

use crate::atlas::cache::entry::Entry;
use crate::atlas::cache::pool::Pool;
use crate::atlas::cache::upload::PendingUpload;
use crate::atlas::key::AtlasKey;
use crate::atlas::sink::queue::TextureQueue;
use crate::atlas::texture::TextureKind;
use crate::atlas::tile::AtlasTile;

pub use crate::atlas::cache::evict::Eviction;
pub use crate::atlas::cache::limits::AtlasLimits;
pub use crate::atlas::cache::report::AtlasReport;
pub use crate::atlas::cache::upload::UploadBytes;

/// A cache of rasterised content packed into textures.
///
/// Each entry is a tile allocated from the pool of its key's [`TextureKind`]. An entry stays
/// cached until it is removed or evicted. Holds taken with [`Atlas::retain`] protect an entry from
/// eviction.
///
/// Texture creations, uploads and destructions are queued. No texture is created, written or
/// destroyed before [`Atlas::flush_uploads`].
///
/// # Generations
///
/// [`Atlas::begin_frame`] starts a new generation. A lookup or insertion marks its entry used in
/// the current generation. Eviction removes unheld entries that are not used in the current
/// generation, oldest generation first.
///
/// ```
/// use iceblink_alloc::atlas::{Atlas, AtlasKey, AtlasLimits, TextureKind};
/// use iceblink::geom::Size;
///
/// let mut atlas = Atlas::new(AtlasLimits::default());
/// let old = AtlasKey::new(1, TextureKind::Mono);
/// let new = AtlasKey::new(2, TextureKind::Mono);
///
/// atlas.begin_frame();
/// atlas.get_or_insert(old, Size::new(4, 4), || vec![0; 16]).unwrap();
///
/// atlas.begin_frame();
/// atlas.get_or_insert(new, Size::new(4, 4), || vec![0; 16]).unwrap();
///
/// let freed = atlas.evict_least_recently_used();
/// assert_eq!(freed.tiles, 1);
/// assert!(!atlas.contains(old));
/// assert!(atlas.contains(new));
/// ```
#[derive(Debug)]
pub struct Atlas {
    limits: AtlasLimits,
    /// One pool per kind, indexed by [`TextureKind::index`].
    pools: [Pool; TextureKind::COUNT],
    /// Entries by slot, `None` where a slot is free.
    // Dense slots let the per-generation use marks live in a bitset.
    entries: Vec<Option<Entry>>,
    free_slots: Vec<u32>,
    /// The slot of each key.
    index: FxHashMap<AtlasKey, u32>,
    /// The slots used in the current generation.
    used: EpochBitset,
    generation: u64,
    hits: u64,
    /// The number of entries with at least one hold.
    // Kept up to date on every hold change, so a report never walks the entries.
    held_tiles: usize,
    /// The tile bytes of the entries with at least one hold.
    held_bytes: u64,
    pending: Vec<PendingUpload>,
    pending_bytes: u64,
    refusals: u64,
    /// The queued texture creations and destructions.
    textures: TextureQueue,
}

impl Atlas {
    /// Creates an empty atlas that allocates within `limits`.
    pub fn new(limits: AtlasLimits) -> Self {
        Self {
            limits,
            pools: TextureKind::ALL.map(Pool::new),
            entries: Vec::new(),
            free_slots: Vec::new(),
            index: FxHashMap::default(),
            used: EpochBitset::new(),
            generation: 0,
            hits: 0,
            held_tiles: 0,
            held_bytes: 0,
            pending: Vec::new(),
            pending_bytes: 0,
            refusals: 0,
            textures: TextureQueue::new(),
        }
    }

    /// Returns the limits the atlas allocates within.
    pub fn limits(&self) -> AtlasLimits {
        self.limits
    }

    /// Sets the resident byte count [`Atlas::evict_to_soft_limit`] frees down to.
    ///
    /// Nothing is freed until that call.
    pub fn set_soft_bytes(&mut self, bytes: Option<u64>) {
        self.limits.soft_bytes = bytes;
    }

    /// Returns the current generation.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Starts a new generation in which no entry is used yet.
    pub fn begin_frame(&mut self) {
        self.generation += 1;
        self.used.bump();
    }

    /// Returns the number of insertions refused since the atlas was created.
    pub fn refusals(&self) -> u64 {
        self.refusals
    }

    /// Returns the number of lookups that found an entry, plus the number of insertions, since the
    /// atlas was created.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Returns the number of texture creations and destructions waiting for
    /// [`Atlas::flush_uploads`].
    pub fn pending_textures(&self) -> usize {
        self.textures.len()
    }

    /// Returns the number of cached entries.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Reports whether no entry is cached.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Reports whether `key` is cached, without marking it used.
    pub fn contains(&self, key: AtlasKey) -> bool {
        self.index.contains_key(&key)
    }

    /// Reports whether `key` is used in the current generation, without marking it used.
    pub fn used_this_frame(&self, key: AtlasKey) -> bool {
        self.index
            .get(&key)
            .and_then(|slot| self.entries[*slot as usize].as_ref())
            .is_some_and(|entry| entry.generation == self.generation)
    }

    /// Returns the tile of every cached entry, in no particular order.
    ///
    /// Tiles of one texture never overlap.
    pub fn tiles(&self) -> impl Iterator<Item = AtlasTile> + '_ {
        self.entries.iter().flatten().map(|entry| entry.tile)
    }

    /// Returns the tile of `key` and marks it used in the current generation.
    pub fn get(&mut self, key: AtlasKey) -> Option<AtlasTile> {
        let slot = *self.index.get(&key)?;
        self.touch(slot);
        self.entries[slot as usize].map(|entry| entry.tile)
    }

    /// Returns the bytes of texture memory the atlas holds, weighted by each pool's format.
    ///
    /// Whole textures count, so the figure falls only when a texture loses its last tile.
    pub fn resident_bytes(&self) -> u64 {
        TextureKind::ALL
            .iter()
            .map(|kind| {
                self.pools[kind.index()].texels() * u64::from(kind.format().bytes_per_texel())
            })
            .sum()
    }

    /// Reports whether no texture has space allocated.
    pub fn is_fully_reclaimed(&self) -> bool {
        self.pools.iter().all(Pool::is_fully_reclaimed)
    }

    /// Marks a slot used and moves its entry to the current generation.
    fn touch(&mut self, slot: u32) {
        self.hits += 1;
        self.used.visit(slot as usize);
        if let Some(entry) = self.entries[slot as usize].as_mut() {
            entry.generation = self.generation;
        }
    }
}
