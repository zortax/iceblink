//! Allocation limits.

use iceblink::geom::{Device, Size};

/// The bounds an [`Atlas`](crate::atlas::Atlas) allocates within.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasLimits {
    /// The side length of a new shared texture, unless its first tile needs more.
    pub texture_size: i32,
    /// The largest side length of a texture or a tile.
    pub max_texture_size: i32,
    /// The number of textures one pool may hold.
    pub max_textures_per_pool: u32,
    /// The resident byte count
    /// [`Atlas::evict_to_soft_limit`](crate::atlas::Atlas::evict_to_soft_limit) frees down to, or
    /// `None` for no limit.
    ///
    /// Allocation is never refused for exceeding it.
    pub soft_bytes: Option<u64>,
}

impl Default for AtlasLimits {
    /// Returns textures of 1024 texels square, at most 4096 square, 16 per pool, and no soft limit.
    // 4096 is the smallest maximum texture dimension of any supported device.
    fn default() -> Self {
        Self {
            texture_size: 1024,
            max_texture_size: 4096,
            max_textures_per_pool: 16,
            soft_bytes: None,
        }
    }
}

impl AtlasLimits {
    /// Returns the limits with the soft limit set to `bytes`.
    pub fn with_soft_bytes(self, bytes: u64) -> Self {
        Self {
            soft_bytes: Some(bytes),
            ..self
        }
    }

    /// Returns the size of the largest tile.
    pub fn largest_tile(self) -> Size<i32, Device> {
        Size::new(self.max_texture_size, self.max_texture_size)
    }

    /// Returns the extent of a new texture for a `requested` tile.
    ///
    /// Each side is [`AtlasLimits::texture_size`], grown to fit the tile and capped at
    /// [`AtlasLimits::max_texture_size`].
    pub fn texture_extent_for(self, requested: Size<i32, Device>) -> Size<i32, Device> {
        let fit = |wanted: i32| wanted.max(self.texture_size).min(self.max_texture_size);
        Size::new(fit(requested.width), fit(requested.height))
    }
}
