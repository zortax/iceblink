//! Chunk keys.

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

use bytemuck::{Pod, Zeroable};

/// A chunk's name, chosen by the client and unique within it.
///
/// One owner has several chunks, told apart by `part`. Two keys are equal when `owner` and `part`
/// are equal, and keys order by `owner`, then `part`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ChunkKey {
    /// The object the chunk belongs to.
    pub owner: u64,
    /// The chunk's position among the owner's chunks.
    pub part: u32,
    /// Ignored, and zero in keys this crate creates.
    pub _pad: u32,
}

impl ChunkKey {
    /// Creates the key of chunk `part` of `owner`.
    pub const fn new(owner: u64, part: u32) -> Self {
        Self {
            owner,
            part,
            _pad: 0,
        }
    }

    const fn name(self) -> (u64, u32) {
        (self.owner, self.part)
    }
}

impl PartialEq for ChunkKey {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for ChunkKey {}

impl Hash for ChunkKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl PartialOrd for ChunkKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChunkKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name().cmp(&other.name())
    }
}
