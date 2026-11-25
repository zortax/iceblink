//! Texture pools, pixel formats and texture identities.

pub mod format;
pub mod kind;

pub use crate::atlas::texture::format::TextureFormat;
pub use crate::atlas::texture::kind::TextureKind;

/// A texture of one pool.
///
/// Indices count within a pool, so textures of different pools can share an index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureId {
    /// The kind of the texture's pool.
    pub kind: TextureKind,
    /// The texture's index within its pool.
    pub index: u32,
}

impl TextureId {
    /// Returns the texture at `index` of the pool of `kind`.
    pub const fn new(kind: TextureKind, index: u32) -> Self {
        Self { kind, index }
    }

    /// Returns the pixel format of the texture's pool.
    pub const fn format(self) -> TextureFormat {
        self.kind.format()
    }
}
