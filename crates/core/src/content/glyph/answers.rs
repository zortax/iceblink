//! What each glyph rasterised to, remembered between lowerings.

use std::collections::VecDeque;

use iceblink::{AtlasTile, GlyphKey, GlyphPlacement};
use iceblink_alloc::atlas::{AtlasError, AtlasKey, UploadBytes};
use rustc_hash::FxHashMap;

use crate::content::glyph::texels::pool_of;
use crate::content::key::TileKey;
use crate::content::tiles::TileStore;

/// The most blank answers and eviction marks kept, each.
const COLD_ANSWERS: usize = 4_096;

/// A glyph's tile and where its bitmap sits from the glyph's origin.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placed {
    pub(crate) tile: AtlasTile,
    pub(crate) key: TileKey,
    pub(crate) placement: GlyphPlacement,
}

/// A rasterised glyph, before it has a tile.
#[derive(Clone, Debug)]
pub(crate) struct RasterAnswer {
    pub(crate) placement: GlyphPlacement,
    pub(crate) size: [u32; 2],
    /// The page texels, or `None` for a glyph with no pixels.
    pub(crate) texels: Option<Vec<u8>>,
}

/// The result of looking a glyph up.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Lookup {
    /// The glyph has a tile.
    Hit(Placed),
    /// The glyph has no pixels.
    Blank,
    /// The glyph must be rasterised.
    Miss,
}

/// One remembered answer.
#[derive(Clone, Copy, Debug)]
struct Remembered {
    /// The tile, or `None` for a glyph with no pixels.
    tile: Option<TileKey>,
    placement: GlyphPlacement,
}

/// The answers of every glyph rasterised so far.
///
/// A glyph with no pixels is remembered too, so a space is rasterised once. Blank answers and
/// the marks of evicted glyphs are bounded by [`COLD_ANSWERS`] each, oldest first.
#[derive(Debug, Default)]
pub(crate) struct GlyphCache {
    entries: FxHashMap<GlyphKey, Remembered>,
    by_tile: FxHashMap<AtlasKey, GlyphKey>,
    blank_order: VecDeque<GlyphKey>,
    evicted: FxHashMap<GlyphKey, u64>,
    evicted_order: VecDeque<(GlyphKey, u64)>,
    eviction_clock: u64,
    /// Glyphs rasterised again after their tile was evicted.
    rebuilt: u64,
}

impl GlyphCache {
    /// Looks `glyph` up, marking its tile used.
    ///
    /// A remembered glyph whose tile was evicted is forgotten and reported as a miss.
    pub(crate) fn lookup(&mut self, tiles: &mut TileStore, glyph: &GlyphKey) -> Lookup {
        if let Some(remembered) = self.entries.get(glyph).copied() {
            let Some(key) = remembered.tile else {
                return Lookup::Blank;
            };
            if let Some(tile) = tiles.get(key) {
                return Lookup::Hit(Placed {
                    tile,
                    key,
                    placement: remembered.placement,
                });
            }
            self.rebuilt += 1;
            self.entries.remove(glyph);
            self.by_tile.remove(&key.atlas());
        } else if self.evicted.remove(glyph).is_some() {
            self.rebuilt += 1;
        }
        Lookup::Miss
    }

    /// Reports whether `glyph` needs rasterising, without marking anything used.
    pub(crate) fn needs_raster(&self, tiles: &TileStore, glyph: &GlyphKey) -> bool {
        match self.entries.get(glyph) {
            Some(remembered) => remembered.tile.is_some_and(|key| !tiles.contains(key)),
            None => true,
        }
    }

    /// Places and remembers a rasterised glyph.
    ///
    /// Returns `Ok(None)` for a blank glyph, and for a glyph the atlas refuses for a reason other
    /// than a full pool, such as one too large for any page. Only the blank glyph is remembered.
    ///
    /// # Errors
    ///
    /// Returns the answer back when the pool is full, so it can be placed again after an
    /// eviction.
    pub(crate) fn remember(
        &mut self,
        tiles: &mut TileStore,
        glyph: GlyphKey,
        answer: RasterAnswer,
    ) -> Result<Option<Placed>, RasterAnswer> {
        let RasterAnswer {
            placement,
            size,
            texels,
        } = answer;
        let Some(texels) = texels else {
            self.remember_blank(glyph, placement);
            return Ok(None);
        };

        let key = tiles.fresh_key(pool_of(glyph.style));
        let mut bytes = Some(UploadBytes::Owned(texels));
        let tile = match tiles.insert(key, size, &mut bytes) {
            Ok(tile) => tile,
            Err(AtlasError::OutOfSpace { .. }) => {
                let texels = match bytes {
                    Some(UploadBytes::Owned(texels)) => Some(texels),
                    _ => None,
                };
                return Err(RasterAnswer {
                    placement,
                    size,
                    texels,
                });
            }
            Err(_) => return Ok(None),
        };

        if let Some(previous) = self.entries.insert(
            glyph,
            Remembered {
                tile: Some(key),
                placement,
            },
        ) && let Some(previous) = previous.tile
        {
            self.by_tile.remove(&previous.atlas());
        }
        self.by_tile.insert(key.atlas(), glyph);
        self.evicted.remove(&glyph);
        Ok(Some(Placed {
            tile,
            key,
            placement,
        }))
    }

    /// Forgets the glyphs whose tiles were removed, and marks them as evicted.
    pub(crate) fn forget_tiles(&mut self, removed: &[AtlasKey]) {
        for atlas_key in removed {
            let Some(glyph) = self.by_tile.remove(atlas_key) else {
                continue;
            };
            if self
                .entries
                .get(&glyph)
                .is_some_and(|entry| entry.tile.map(TileKey::atlas) == Some(*atlas_key))
            {
                self.entries.remove(&glyph);
                self.remember_eviction(glyph);
            }
        }
    }

    /// Forgets everything.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.by_tile.clear();
        self.blank_order.clear();
        self.evicted.clear();
        self.evicted_order.clear();
    }

    /// Returns the number of remembered answers, blank ones included.
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the glyphs rasterised again after their tile was evicted.
    pub(crate) fn rebuilt(&self) -> u64 {
        self.rebuilt
    }

    fn remember_blank(&mut self, glyph: GlyphKey, placement: GlyphPlacement) {
        self.entries.insert(
            glyph,
            Remembered {
                tile: None,
                placement,
            },
        );

        self.blank_order.push_back(glyph);
        while self.blank_order.len() > COLD_ANSWERS {
            let Some(oldest) = self.blank_order.pop_front() else {
                break;
            };
            if self
                .entries
                .get(&oldest)
                .is_some_and(|entry| entry.tile.is_none())
            {
                self.entries.remove(&oldest);
            }
        }
    }

    fn remember_eviction(&mut self, glyph: GlyphKey) {
        self.eviction_clock = self.eviction_clock.wrapping_add(1);
        let token = self.eviction_clock;
        self.evicted.insert(glyph, token);
        self.evicted_order.push_back((glyph, token));

        // A glyph evicted again leaves its older token in the queue, which bounds the queue too.
        while self.evicted.len() > COLD_ANSWERS || self.evicted_order.len() > 2 * COLD_ANSWERS {
            let Some((oldest, token)) = self.evicted_order.pop_front() else {
                break;
            };
            if self.evicted.get(&oldest) == Some(&token) {
                self.evicted.remove(&oldest);
            }
        }
    }
}
