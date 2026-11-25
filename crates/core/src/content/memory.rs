//! What a content cache holds, in bytes.

/// The bytes a [`ContentCache`](crate::content::ContentCache) holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentMemory {
    /// The bytes of the atlas pages.
    pub atlas_bytes: u64,
    /// The tile bytes of the pinned tiles, which eviction keeps.
    pub atlas_pinned_bytes: u64,
}
