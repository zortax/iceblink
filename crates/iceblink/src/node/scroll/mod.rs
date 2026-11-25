//! Scroll nodes.

mod behaviour;

use crate::geometry::LocalRect;
use crate::id::TransformId;

pub use self::behaviour::{ScrollBehaviour, ScrollPhysics, SnapKind, SnapPoint, Sources};

/// A scrollable region, whose offset the transform nodes that refer to it apply.
///
/// A scroll node does not clip. Content clipped to the port is drawn through a clip node of its
/// own.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollNode {
    /// The transform node the port and the offsets are measured in.
    pub space: TransformId,
    /// The visible rectangle.
    pub port: LocalRect,
    /// The size of the content. The largest offset is `max(content - port size, 0)` per axis.
    pub content: [f32; 2],
    /// How far the content is scrolled, clamped to `[0, largest offset]` per axis when written.
    pub offset: [f32; 2],
    /// How the compositor scrolls the node on its own. `None` scrolls only by client writes.
    pub behaviour: Option<ScrollBehaviour>,
}
