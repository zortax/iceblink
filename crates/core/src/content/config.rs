//! The limits a content cache allocates pages within.

use iceblink::RenderCapabilities;
use iceblink_alloc::atlas::AtlasLimits;

/// The sizes and counts of the pages a [`ContentCache`](crate::content::ContentCache) creates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentConfig {
    /// The side of a new shared page, unless its first tile needs more.
    pub page_size: u32,
    /// The largest side of any page, at most the renderer's `max_texture_size`.
    pub max_page_size: u32,
    /// The number of pages each of the four pools may hold.
    pub pages_per_pool: u32,
}

impl Default for ContentConfig {
    /// Returns shared pages of 1024 texels, at most 4096 texels on a side and 16 pages per pool.
    fn default() -> Self {
        Self {
            page_size: 1024,
            max_page_size: 4096,
            pages_per_pool: 16,
        }
    }
}

impl ContentConfig {
    /// Returns the default configuration with pages no larger than the renderer allows.
    pub fn for_capabilities(caps: &RenderCapabilities) -> Self {
        let default = Self::default();
        let max_page_size = caps.max_texture_size.max(1);
        Self {
            page_size: default.page_size.min(max_page_size),
            max_page_size,
            ..default
        }
    }

    /// Returns the atlas limits of this configuration.
    pub(crate) fn limits(self) -> AtlasLimits {
        let clamp = |value: u32| i32::try_from(value.max(1)).unwrap_or(i32::MAX);
        AtlasLimits {
            texture_size: clamp(self.page_size.min(self.max_page_size)),
            max_texture_size: clamp(self.max_page_size),
            max_textures_per_pool: self.pages_per_pool.max(1),
            soft_bytes: None,
        }
    }

    /// Returns the largest side of an image that shares a page with other content.
    pub(crate) fn shared_image_side(self) -> u32 {
        self.page_size.min(self.max_page_size) / 2
    }
}
