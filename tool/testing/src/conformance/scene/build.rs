//! The values the scene cases build: transform nodes, chunk props, quads, effects and device
//! rectangles.

use std::sync::Arc;

use iceblink::geom::{Point, Rect, Size};
use iceblink::{
    Anchor, BlendMode, BorderStyle, ChunkFlags, ChunkKey, ChunkProps, ClipRef, DeviceRect,
    EffectNode, Matrix4, PaintRef, Quad, ROUND, SpaceRef, Transaction, TransformId, TransformNode,
};

/// A transform node with an identity matrix under `parent`.
pub(crate) fn identity(parent: Option<TransformId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    }
}

/// Chunk props in `space` with no clip.
pub(crate) fn props(space: TransformId) -> ChunkProps {
    ChunkProps {
        space,
        clip: None,
        flags: ChunkFlags::default(),
    }
}

/// A key of `owner`'s first chunk.
pub(crate) fn key(owner: u64) -> ChunkKey {
    ChunkKey::new(owner, 0)
}

/// A borderless quad over `bounds` filled with the inline colour `rgba`, in the chunk's space and
/// clip.
pub(crate) fn quad(bounds: [f32; 4], rgba: [u8; 4]) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill: PaintRef::color(rgba),
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}

/// Records chunk `key` with one quad per `(bounds, colour)` into `tx`.
pub(crate) fn record_quads(
    tx: &mut Transaction,
    key: ChunkKey,
    props: ChunkProps,
    quads: &[([f32; 4], [u8; 4])],
) {
    let mut chunk = tx.record(key, props);
    for &(bounds, rgba) in quads {
        chunk.quad(quad(bounds, rgba));
    }
}

/// An effect with an opacity and nothing else.
pub(crate) fn faded(opacity: f32) -> EffectNode {
    EffectNode {
        clip: None,
        opacity,
        blend: BlendMode::NORMAL,
        filters: Arc::from(Vec::new()),
        isolate: false,
    }
}

/// The device rectangle `[x, y, width, height]`.
pub(crate) fn device(x: i32, y: i32, width: i32, height: i32) -> DeviceRect {
    Rect::new(Point::new(x, y), Size::new(width, height))
}

/// The whole pixels a local rectangle under an identity matrix touches.
pub(crate) fn pixels([x, y, width, height]: [f32; 4]) -> DeviceRect {
    Rect::from_corners(
        Point::new(x.floor() as i32, y.floor() as i32),
        Point::new((x + width).ceil() as i32, (y + height).ceil() as i32),
    )
}
