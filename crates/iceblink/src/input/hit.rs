//! Hit test results.

use crate::chunk::ChunkKey;
use crate::id::ClientId;

/// An item under a device point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitResult {
    /// The client that recorded the item.
    pub client: ClientId,
    /// The chunk the item belongs to.
    pub chunk: ChunkKey,
    /// The item's index in the chunk, counted across kinds in record order.
    pub item: u32,
    /// The point in the item's local space.
    pub local: [f32; 2],
}
