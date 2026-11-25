//! The textures of one kind, each with a bucketed shelf allocator.

use core::fmt;

use etagere::{AllocId, Allocation, BucketedAtlasAllocator, size2};
use iceblink::geom::{Device, Point, Rect, Size};

use crate::atlas::cache::limits::AtlasLimits;
use crate::atlas::error::AtlasError;
use crate::atlas::sink::queue::TextureQueue;
use crate::atlas::texture::{TextureId, TextureKind};
use crate::atlas::tile::TileId;

/// A tile's texture, allocation and rectangle.
pub(crate) type Placement = (TextureId, TileId, Rect<i32, Device>);

/// One texture and the allocator of its space.
struct PoolTexture {
    allocator: BucketedAtlasAllocator,
    size: Size<i32, Device>,
    /// The number of allocated tiles. The texture is destroyed when it reaches zero.
    live: u32,
}

/// Every texture of one [`TextureKind`].
///
/// The slot of a destroyed texture is reused by the next new texture, so texture indices stay
/// dense.
pub(crate) struct Pool {
    kind: TextureKind,
    /// Textures by index, `None` where a slot is free.
    textures: Vec<Option<PoolTexture>>,
}

impl fmt::Debug for Pool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Pool")
            .field("kind", &self.kind)
            .field("textures", &self.live_textures())
            .field("fully_reclaimed", &self.is_fully_reclaimed())
            .finish()
    }
}

impl Pool {
    /// Creates an empty pool of `kind`.
    pub(crate) fn new(kind: TextureKind) -> Self {
        Self {
            kind,
            textures: Vec::new(),
        }
    }

    /// Returns the number of existing textures.
    pub(crate) fn live_textures(&self) -> usize {
        self.textures.iter().flatten().count()
    }

    /// Returns the texels of every existing texture.
    pub(crate) fn texels(&self) -> u64 {
        self.textures
            .iter()
            .flatten()
            .map(|texture| texture.size.width as u64 * texture.size.height as u64)
            .sum()
    }

    /// Reports whether no texture has space allocated.
    pub(crate) fn is_fully_reclaimed(&self) -> bool {
        self.textures
            .iter()
            .flatten()
            .all(|texture| texture.allocator.allocated_space() == 0)
    }

    /// Allocates a tile of `size`, queueing a new texture if no existing one has room.
    ///
    /// Existing textures are tried from the highest index down.
    ///
    /// # Errors
    ///
    /// [`AtlasError::TooLarge`] if `size` exceeds the largest texture, and
    /// [`AtlasError::OutOfSpace`] if `size` is not positive or the pool is full and may not grow.
    pub(crate) fn allocate(
        &mut self,
        size: Size<i32, Device>,
        limits: AtlasLimits,
        queue: &mut TextureQueue,
    ) -> Result<Placement, AtlasError> {
        check_size(size, limits)?;

        for index in (0..self.textures.len()).rev() {
            let Some(texture) = self.textures[index].as_mut() else {
                continue;
            };
            if let Some(allocation) = texture.allocator.allocate(size2(size.width, size.height)) {
                texture.live += 1;
                return Ok(self.placement(index, allocation, size));
            }
        }

        let index = self.push_texture(size, limits, queue)?;
        let texture = self.textures[index]
            .as_mut()
            .expect("the slot was just filled");
        let allocation = texture
            .allocator
            .allocate(size2(size.width, size.height))
            .ok_or(AtlasError::OutOfSpace { requested: size })?;
        texture.live += 1;
        Ok(self.placement(index, allocation, size))
    }

    /// Allocates a tile that covers a new texture of exactly `size` with `mip_levels` levels.
    ///
    /// # Errors
    ///
    /// [`AtlasError::TooLarge`] if `size` exceeds the largest texture, and
    /// [`AtlasError::OutOfSpace`] if `size` is not positive or the pool may not grow.
    pub(crate) fn allocate_exact(
        &mut self,
        size: Size<i32, Device>,
        mip_levels: u32,
        limits: AtlasLimits,
        queue: &mut TextureQueue,
    ) -> Result<Placement, AtlasError> {
        check_size(size, limits)?;
        if self.live_textures() as u32 >= limits.max_textures_per_pool {
            return Err(AtlasError::OutOfSpace { requested: size });
        }

        let index = self.free_slot();
        // Allocate before queueing the creation, so a refusal leaves no orphaned texture.
        let mut allocator = BucketedAtlasAllocator::new(size2(size.width, size.height));
        let allocation = allocator
            .allocate(size2(size.width, size.height))
            .ok_or(AtlasError::OutOfSpace { requested: size })?;
        let id = TextureId::new(self.kind, index as u32);
        queue.create_with_mips(id, size, self.kind.format(), mip_levels);
        self.textures[index] = Some(PoolTexture {
            allocator,
            size,
            live: 1,
        });
        Ok(self.placement(index, allocation, size))
    }

    /// Frees a tile, queueing the destruction of its texture if no tile is left.
    pub(crate) fn deallocate(
        &mut self,
        texture: TextureId,
        tile: TileId,
        queue: &mut TextureQueue,
    ) {
        let Some(slot) = self.textures.get_mut(texture.index as usize) else {
            return;
        };
        let Some(held) = slot.as_mut() else { return };
        held.allocator.deallocate(AllocId::deserialize(tile.0));
        held.live = held.live.saturating_sub(1);
        if held.live == 0 {
            *slot = None;
            queue.destroy(texture);
        }
    }

    /// Queues the destruction of every texture.
    pub(crate) fn clear(&mut self, queue: &mut TextureQueue) {
        for (index, slot) in self.textures.iter_mut().enumerate() {
            if slot.take().is_some() {
                queue.destroy(TextureId::new(self.kind, index as u32));
            }
        }
        self.textures.clear();
    }

    /// Queues a new texture with room for `size` and returns its slot.
    fn push_texture(
        &mut self,
        size: Size<i32, Device>,
        limits: AtlasLimits,
        queue: &mut TextureQueue,
    ) -> Result<usize, AtlasError> {
        if self.live_textures() as u32 >= limits.max_textures_per_pool {
            return Err(AtlasError::OutOfSpace { requested: size });
        }
        let extent = limits.texture_extent_for(size);
        let index = self.free_slot();
        let id = TextureId::new(self.kind, index as u32);
        queue.create(id, extent, self.kind.format());
        self.textures[index] = Some(PoolTexture {
            allocator: BucketedAtlasAllocator::new(size2(extent.width, extent.height)),
            size: extent,
            live: 0,
        });
        Ok(index)
    }

    /// Returns the placement of `allocation` in texture `index`, sized to the requested `size`.
    fn placement(
        &self,
        index: usize,
        allocation: Allocation,
        size: Size<i32, Device>,
    ) -> Placement {
        (
            TextureId::new(self.kind, index as u32),
            TileId(allocation.id.serialize()),
            rect_of(allocation.rectangle, size),
        )
    }

    /// Returns the lowest free texture slot, appending one if none is free.
    fn free_slot(&mut self) -> usize {
        let index = self
            .textures
            .iter()
            .position(Option::is_none)
            .unwrap_or(self.textures.len());
        if index == self.textures.len() {
            self.textures.push(None);
        }
        index
    }
}

/// Rejects a tile size that is not positive or exceeds the largest texture.
fn check_size(size: Size<i32, Device>, limits: AtlasLimits) -> Result<(), AtlasError> {
    if size.width <= 0 || size.height <= 0 {
        return Err(AtlasError::OutOfSpace { requested: size });
    }
    if size.width > limits.max_texture_size || size.height > limits.max_texture_size {
        return Err(AtlasError::TooLarge {
            requested: size,
            limit: limits.largest_tile(),
        });
    }
    Ok(())
}

/// Returns the requested `size` at the origin of an allocation.
// Shelf packing rounds allocations up, and the slack stays unused.
fn rect_of(allocation: etagere::Rectangle, size: Size<i32, Device>) -> Rect<i32, Device> {
    Rect::new(Point::new(allocation.min.x, allocation.min.y), size)
}
