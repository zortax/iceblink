//! The nodes and geometry of the list.

use std::sync::Arc;

use iceblink::{
    Anchor, ChunkFlags, ChunkKey, ChunkProps, ClipNode, ClipShape, Matrix4, ROUND, ScrollBehaviour,
    ScrollId, ScrollNode, ScrollPhysics, SnapKind, Sources, TransformId, TransformNode,
};

use super::{MARGIN, ROW, ROWS, TOP};

/// Returns a transform node under `parent` with `local`, scrolled by `scroll` if given.
pub(super) fn node(
    parent: Option<TransformId>,
    local: Matrix4,
    scroll: Option<ScrollId>,
) -> TransformNode {
    TransformNode {
        parent,
        local,
        scroll,
        anchor: Anchor::Scrolls,
    }
}

/// Returns chunk props in `space` with no clip.
pub(super) fn props(space: TransformId) -> ChunkProps {
    ChunkProps {
        space,
        clip: None,
        flags: ChunkFlags::default(),
    }
}

/// Returns the key of rows chunk `chunk`.
pub(super) fn rows_key(chunk: usize) -> ChunkKey {
    ChunkKey::new(100 + chunk as u64, 0)
}

/// Returns the matrix that maps logical pixels to device pixels at `scale`.
pub(super) fn scaled(scale: f32) -> Matrix4 {
    Matrix4::scale(scale, scale, 1.0)
}

/// Returns the port of a window of `size`.
pub(super) fn port(size: [f32; 2]) -> [f32; 4] {
    [
        MARGIN,
        TOP,
        (size[0] - 2.0 * MARGIN).max(1.0),
        (size[1] - TOP - MARGIN).max(1.0),
    ]
}

/// Returns the scroll node of the port of a window of `size`, scrolled to `offset`.
pub(super) fn scroll_node(space: TransformId, size: [f32; 2], offset: f32) -> ScrollNode {
    let port = port(size);
    ScrollNode {
        space,
        port,
        content: [port[2], ROWS as f32 * ROW + 8.0],
        offset: [0.0, offset],
        behaviour: Some(ScrollBehaviour {
            physics: ScrollPhysics::Kinetic,
            snap: Arc::from([]),
            snap_kind: SnapKind::Mandatory,
            bounce: Sources::ALL,
            user_scrollable: [false, true],
            chain: Sources::NONE,
        }),
    }
}

/// Returns the clip to the port of a window of `size`.
pub(super) fn port_clip(space: TransformId, size: [f32; 2]) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: port(size),
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}
