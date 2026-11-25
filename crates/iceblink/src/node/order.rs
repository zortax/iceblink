//! Order tree nodes.

use crate::chunk::ChunkKey;
use crate::id::{EffectId, OrderId};

/// A node of the order tree, which holds a sequence of entries.
///
/// The paint order of an output is the depth-first traversal of its root. An isolating effect
/// wraps the node's subtree in a group.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OrderNode {
    /// The effect applied to the node's subtree.
    pub effect: Option<EffectId>,
}

/// An entry of an order node's sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrderEntry {
    /// The items of a chunk, in record order. A chunk not yet recorded draws nothing.
    Chunk(ChunkKey),
    /// The subtree of another order node.
    Node(OrderId),
}
