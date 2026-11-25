//! The default property state of a chunk.

use crate::bits::bit_set;
use crate::id::{ClipId, TransformId};

/// The default property state of a chunk's items.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkProps {
    /// The transform node of every item whose `space` is `SpaceRef::INHERIT`.
    pub space: TransformId,
    /// The clip chain of every item whose `clip` is `ClipRef::INHERIT`, or `None` for no clip.
    pub clip: Option<ClipId>,
    /// How the chunk takes part in hit testing.
    pub flags: ChunkFlags,
}

bit_set! {
    /// How a chunk takes part in hit testing.
    pub struct ChunkFlags(u8) {
        /// Hit testing passes through the chunk's items.
        HIT_TRANSPARENT = 1 << 0,
    }
}
