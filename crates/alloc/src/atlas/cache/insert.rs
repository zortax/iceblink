//! Inserting entries.

use iceblink::geom::{Device, Point, Rect, Size};

use crate::atlas::cache::Atlas;
use crate::atlas::cache::entry::Entry;
use crate::atlas::cache::upload::{PendingUpload, UploadBytes};
use crate::atlas::error::AtlasError;
use crate::atlas::key::AtlasKey;
use crate::atlas::tile::AtlasTile;

impl Atlas {
    /// Returns the tile of `key`, allocating one and queueing the bytes `build` returns if `key` is
    /// not cached.
    ///
    /// `build` is called only if `key` is not cached. It returns `size` texels in the pool's format
    /// as tightly packed rows, top row first. A shared `Arc<Vec<u8>>` is queued without a copy. The
    /// entry is marked used in the current generation.
    ///
    /// # Errors
    ///
    /// Each error counts as a refusal:
    ///
    /// - [`AtlasError::TooLarge`] if `size` exceeds
    ///   [`AtlasLimits::max_texture_size`](crate::atlas::AtlasLimits::max_texture_size).
    /// - [`AtlasError::OutOfSpace`] if `size` is not positive, or if every texture of the pool is
    ///   full and the pool may not grow.
    /// - [`AtlasError::WrongByteCount`] if the bytes `build` returns do not match `size`.
    pub fn get_or_insert<B: Into<UploadBytes>>(
        &mut self,
        key: AtlasKey,
        size: Size<i32, Device>,
        build: impl FnOnce() -> B,
    ) -> Result<AtlasTile, AtlasError> {
        if let Some(tile) = self.get(key) {
            return Ok(tile);
        }

        let kind = key.kind();
        let (texture, tile_id, bounds) =
            match self.pools[kind.index()].allocate(size, self.limits, &mut self.textures) {
                Ok(placed) => placed,
                Err(error) => {
                    self.refusals += 1;
                    return Err(error);
                }
            };

        let bytes: UploadBytes = build().into();
        let expected = kind
            .format()
            .bytes_for(size.width.max(0) as u32, size.height.max(0) as u32);
        if bytes.len() as u64 != expected {
            self.pools[kind.index()].deallocate(texture, tile_id, &mut self.textures);
            self.refusals += 1;
            return Err(AtlasError::WrongByteCount {
                size,
                expected,
                actual: bytes.len() as u64,
            });
        }

        self.pending_bytes += bytes.len() as u64;
        self.pending.push(PendingUpload {
            texture,
            tile: tile_id,
            mip: 0,
            bounds,
            bytes,
        });

        let tile = AtlasTile {
            texture,
            tile: tile_id,
            bounds,
        };
        self.insert_entry(key, tile, size);
        Ok(tile)
    }

    /// Returns the tile of `key`, creating a texture of exactly `size` with levels of detail if
    /// `key` is not cached.
    ///
    /// The tile covers its whole texture. `build` is called only if `key` is not cached, and
    /// returns level zero followed by each further level. Each level is half the extent of the
    /// previous one, rounded down and at least one texel, and is queued as its own upload. The
    /// entry is marked used in the current generation, and is held and evicted like any other.
    ///
    /// # Errors
    ///
    /// Each error counts as a refusal:
    ///
    /// - [`AtlasError::WrongByteCount`] for the first level whose bytes do not match its extent.
    /// - [`AtlasError::TooLarge`] if `size` exceeds
    ///   [`AtlasLimits::max_texture_size`](crate::atlas::AtlasLimits::max_texture_size).
    /// - [`AtlasError::OutOfSpace`] if `size` is not positive, or if the pool already holds
    ///   [`AtlasLimits::max_textures_per_pool`](crate::atlas::AtlasLimits::max_textures_per_pool)
    ///   textures.
    pub fn insert_standalone<B: Into<UploadBytes>>(
        &mut self,
        key: AtlasKey,
        size: Size<i32, Device>,
        build: impl FnOnce() -> Vec<B>,
    ) -> Result<AtlasTile, AtlasError> {
        if let Some(tile) = self.get(key) {
            return Ok(tile);
        }

        let kind = key.kind();
        let levels: Vec<UploadBytes> = build().into_iter().map(Into::into).collect();
        for (mip, bytes) in levels.iter().enumerate() {
            let extent = mip_extent(size, mip as u32);
            let expected = kind
                .format()
                .bytes_for(extent.width.max(0) as u32, extent.height.max(0) as u32);
            if bytes.len() as u64 != expected {
                self.refusals += 1;
                return Err(AtlasError::WrongByteCount {
                    size: extent,
                    expected,
                    actual: bytes.len() as u64,
                });
            }
        }

        let mip_levels = (levels.len() as u32).max(1);
        let (texture, tile_id, bounds) = match self.pools[kind.index()].allocate_exact(
            size,
            mip_levels,
            self.limits,
            &mut self.textures,
        ) {
            Ok(placed) => placed,
            Err(error) => {
                self.refusals += 1;
                return Err(error);
            }
        };

        for (mip, bytes) in levels.into_iter().enumerate() {
            let extent = mip_extent(size, mip as u32);
            self.pending_bytes += bytes.len() as u64;
            self.pending.push(PendingUpload {
                texture,
                tile: tile_id,
                mip: mip as u32,
                bounds: Rect::new(Point::new(0, 0), extent),
                bytes,
            });
        }

        let tile = AtlasTile {
            texture,
            tile: tile_id,
            bounds,
        };
        self.insert_entry(key, tile, size);
        Ok(tile)
    }

    /// Stores a new entry in a free slot and marks it used.
    fn insert_entry(&mut self, key: AtlasKey, tile: AtlasTile, size: Size<i32, Device>) {
        let entry = Entry {
            key,
            tile,
            size,
            refs: 0,
            generation: self.generation,
        };
        let slot = match self.free_slots.pop() {
            Some(slot) => {
                self.entries[slot as usize] = Some(entry);
                slot
            }
            None => {
                self.entries.push(Some(entry));
                (self.entries.len() - 1) as u32
            }
        };
        self.index.insert(key, slot);
        self.touch(slot);
    }
}

/// Returns the extent of level `mip`: half the previous extent per level, rounded down and at
/// least one.
fn mip_extent(size: Size<i32, Device>, mip: u32) -> Size<i32, Device> {
    Size::new((size.width >> mip).max(1), (size.height >> mip).max(1))
}
