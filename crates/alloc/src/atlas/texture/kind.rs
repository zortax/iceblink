//! The pools a tile can be allocated from.

use crate::atlas::texture::format::TextureFormat;

/// The pool a tile is allocated from.
///
/// Each pool has its own textures and a fixed pixel format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TextureKind {
    /// Single-channel coverage, such as glyph masks and vector shapes rasterised as masks.
    Mono,
    /// Coverage for subpixel text, one value per colour channel.
    Subpixel,
    /// Full colour rasterised at its drawn size, such as emoji and colour glyphs.
    Color,
    /// Full colour scaled to its drawn size and sampled with filtering, such as decoded images.
    Image,
}

impl TextureKind {
    /// The number of kinds.
    pub const COUNT: usize = 4;

    /// Every kind, in pool order.
    pub const ALL: [Self; Self::COUNT] = [Self::Mono, Self::Subpixel, Self::Color, Self::Image];

    /// Returns the pixel format of the kind's textures.
    // Fixed formats need no channel swizzle on upload, and one byte per coverage texel quarters the
    // upload bandwidth of text.
    pub const fn format(self) -> TextureFormat {
        match self {
            Self::Mono => TextureFormat::R8Unorm,
            Self::Subpixel | Self::Color | Self::Image => TextureFormat::Rgba8Unorm,
        }
    }

    /// Returns the kind's position in [`TextureKind::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::Mono => 0,
            Self::Subpixel => 1,
            Self::Color => 2,
            Self::Image => 3,
        }
    }
}
