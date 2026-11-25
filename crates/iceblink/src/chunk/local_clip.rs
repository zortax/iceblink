//! Clips that belong to one chunk.

use crate::id::ClipId;
use crate::node::ClipShape;

/// A clip that belongs to one chunk and moves with its offset.
///
/// Its shape is measured in the chunk's space, translated by the chunk's offset. Local clips are
/// numbered from zero in the order they are added, and a chunk holds at most 2²⁴ of them.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalClip {
    /// The next link outwards.
    pub parent: LocalParent,
    /// The region the clip admits.
    pub shape: ClipShape,
}

/// The next link outwards of a chunk-local clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LocalParent {
    /// The chunk's clip, `ChunkProps::clip`, or nothing when it has none.
    Chunk,
    /// An earlier chunk-local clip, by its index.
    Local(u32),
    /// A clip node.
    Node(ClipId),
    /// No parent.
    None,
}
