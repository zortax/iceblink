//! Builders shared by the transaction, id and damage tests.

// Every test binary compiles this module and uses a subset of it.
#![allow(dead_code)]

pub(crate) mod damage;

use iceblink::{
    Anchor, ChunkFlags, ChunkProps, ClientId, ClipRef, IdSpace, Matrix4, PaintRef, Quad, ROUND,
    SpaceRef, Transaction, TransformNode,
};

/// The id space of client 1.
pub(crate) fn ids() -> IdSpace {
    IdSpace::new(ClientId(1))
}

/// A root transform node with the identity matrix.
pub(crate) fn root_node() -> TransformNode {
    TransformNode {
        parent: None,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    }
}

/// Default props under a new root transform node of `transaction`.
pub(crate) fn props(transaction: &mut Transaction) -> ChunkProps {
    ChunkProps {
        space: transaction.create_transform(root_node()),
        clip: None,
        flags: ChunkFlags::default(),
    }
}

/// A borderless square quad at `x` with no paint.
pub(crate) fn quad(x: f32) -> Quad {
    Quad {
        bounds: [x, 0.0, 10.0, 10.0],
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill: PaintRef::NONE,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: Default::default(),
        _pad: [0; 3],
    }
}
