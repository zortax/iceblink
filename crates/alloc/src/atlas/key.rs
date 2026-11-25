//! Cache keys.

use crate::atlas::texture::TextureKind;

/// The key content is cached under: an opaque handle and the kind of the content's pool.
///
/// The handle is any `u64`, typically a hash of everything that affects the pixels. The atlas
/// compares handles and never interprets them. Keys with the same handle and different kinds are
/// different keys.
///
/// ```
/// use iceblink_alloc::atlas::{AtlasKey, TextureKind};
///
/// let glyph = AtlasKey::new(0x1234, TextureKind::Mono);
/// assert_eq!(glyph, AtlasKey::new(0x1234, TextureKind::Mono));
/// assert_ne!(glyph, AtlasKey::new(0x1234, TextureKind::Color));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AtlasKey {
    kind: TextureKind,
    handle: u64,
}

impl AtlasKey {
    /// Returns the key of `handle` in the pool of `kind`.
    pub const fn new(handle: u64, kind: TextureKind) -> Self {
        Self { kind, handle }
    }

    /// Returns the opaque handle.
    pub const fn handle(self) -> u64 {
        self.handle
    }

    /// Returns the kind of the content's pool.
    pub const fn kind(self) -> TextureKind {
        self.kind
    }
}
