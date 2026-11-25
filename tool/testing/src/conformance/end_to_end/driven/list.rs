//! The scene of the driven list and the client's changes to it.

use std::sync::Arc;

use iceblink::{
    Anchor, BlendMode, BorderStyle, ChunkFlags, ChunkKey, ChunkProps, ClipId, ClipNode, ClipRef,
    ClipShape, Color, CompositorEvent, EffectId, EffectNode, Matrix4, OrderEntry, OrderNode,
    OutputId, Paint, PaintId, PaintKind, PaintRef, Quad, ROUND, ScrollBehaviour, ScrollId,
    ScrollNode, ScrollPhysics, SnapKind, Sources, SpaceRef, ThumbAnchor, Transaction, TransformId,
    TransformNode,
};

/// The scroll port.
const PORT: [f32; 4] = [8.0, 8.0, 96.0, 112.0];

/// How many rows the content holds, 30 apart.
pub(crate) const ROWS: u32 = 40;

/// The ids of the scene in one compositor.
#[derive(Clone, Copy)]
pub(crate) struct List {
    scroll: ScrollId,
    content: TransformId,
    clip: ClipId,
    tint: PaintId,
    fade: EffectId,
}

fn props(space: TransformId, clip: Option<ClipId>) -> ChunkProps {
    ChunkProps {
        space,
        clip,
        flags: ChunkFlags::default(),
    }
}

fn identity(parent: TransformId) -> TransformNode {
    TransformNode {
        parent: Some(parent),
        ..identity_root()
    }
}

fn identity_root() -> TransformNode {
    TransformNode {
        parent: None,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    }
}

/// Returns the bounds of row `row` in the content's space.
fn row_bounds(row: u32) -> [f32; 4] {
    [12.0, 10.0 + row as f32 * 30.0, 88.0, 24.0]
}

fn fade(opacity: f32) -> EffectNode {
    EffectNode {
        clip: None,
        opacity,
        blend: BlendMode::NORMAL,
        filters: Arc::from([]),
        isolate: false,
    }
}

/// Builds the scene in `tx` as the content of `output`, and returns its ids.
pub(crate) fn list(tx: &mut Transaction, output: OutputId) -> List {
    let space = tx.create_transform(identity_root());
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));

    let mut at = 0;
    let mut append = |tx: &mut Transaction, entry: OrderEntry| {
        tx.splice(root, at, 0, &[entry]);
        at += 1;
    };

    let white = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
    let grey = tx.create_paint(Paint::Solid(Color([0.4, 0.4, 0.45, 1.0])));
    let tint = tx.create_paint(Paint::Solid(Color([0.5, 0.0, 0.0, 1.0])));
    let backing = ChunkKey::new(1, 0);
    let mut chunk = tx.record(backing, props(space, None));
    let fill = chunk.paint_ref(PaintKind::SOLID, white);
    chunk.quad(quad_of([0.0, 0.0, 128.0, 128.0], fill));
    append(tx, OrderEntry::Chunk(backing));

    let scroll = tx.create_scroll(ScrollNode {
        space,
        port: PORT,
        content: [PORT[2], 10.0 + ROWS as f32 * 30.0],
        offset: [0.0; 2],
        behaviour: Some(ScrollBehaviour {
            physics: ScrollPhysics::Kinetic,
            snap: Arc::from([]),
            snap_kind: SnapKind::Mandatory,
            bounce: Sources::ALL,
            user_scrollable: [false, true],
            chain: Sources::ALL,
        }),
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(space)
    });
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: PORT,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });

    let fade_effect = tx.create_effect(fade(1.0));
    let rows = tx.create_order(OrderNode::default());
    append(tx, OrderEntry::Node(rows));
    for row in 0..ROWS {
        let key = ChunkKey::new(2, row);
        let mut chunk = tx.record(key, props(content, Some(clip)));
        let fill = chunk.paint_ref(PaintKind::SOLID, if row % 4 == 1 { tint } else { grey });
        chunk.quad(quad_of(row_bounds(row), fill));
        if row % 4 == 3 {
            let group = tx.create_order(OrderNode {
                effect: Some(fade_effect),
            });
            tx.splice(rows, row, 0, &[OrderEntry::Node(group)]);
            tx.splice(group, 0, 0, &[OrderEntry::Chunk(key)]);
        } else {
            tx.splice(rows, row, 0, &[OrderEntry::Chunk(key)]);
        }
    }

    let thumb = tx.create_transform(TransformNode {
        anchor: Anchor::Thumb(ThumbAnchor {
            scroll,
            track: [0.0, PORT[3] - 20.0],
        }),
        ..identity(space)
    });
    let bar = ChunkKey::new(3, 0);
    let mut chunk = tx.record(bar, props(thumb, None));
    let fill = chunk.paint_ref(PaintKind::SOLID, grey);
    chunk.quad(quad_of([108.0, PORT[1], 8.0, 20.0], fill));
    append(tx, OrderEntry::Chunk(bar));

    List {
        scroll,
        content,
        clip,
        tint,
        fade: fade_effect,
    }
}

/// How often, in frames, the client records rows again and acknowledges or writes the offset.
#[derive(Clone, Copy)]
pub(crate) struct Cadence {
    /// The rows recorded again with an inline colour, as the first and the step to the next.
    pub(crate) rows: (u32, usize),
    /// The rows are recorded again in frames divisible by it.
    pub(crate) record: u32,
    /// The newest offset is acknowledged in frames divisible by it.
    pub(crate) acknowledge: u32,
    /// The client writes the newest offset moved by 2 pixels in frames divisible by it, in place
    /// of the acknowledgement.
    pub(crate) jump: Option<u32>,
}

/// Writes the client's changes of frame `frame` into `tx`, at `cadence`, acknowledging or moving
/// the newest offset of `events`.
pub(crate) fn change(
    tx: &mut Transaction,
    list: List,
    frame: u32,
    cadence: Cadence,
    events: &[CompositorEvent],
) {
    let level = (frame / 3 % 4) as f32;
    tx.set_paint(
        list.tint,
        Paint::Solid(Color([0.4 + 0.2 * level, 0.1, 0.1, 1.0])),
    );
    if frame.is_multiple_of(2) {
        tx.set_effect(list.fade, fade(0.3 + 0.2 * (frame / 2 % 4) as f32));
    }
    if frame.is_multiple_of(cadence.record) {
        let shade = (40 + frame * 37 % 200) as u8;
        let (first, step) = cadence.rows;
        for row in (first..ROWS).step_by(step) {
            let key = ChunkKey::new(2, row);
            let mut chunk = tx.record(key, props(list.content, Some(list.clip)));
            chunk.quad(quad_of(
                row_bounds(row),
                PaintRef::color([shade, 120, 60, 255]),
            ));
        }
    }
    let newest = events.iter().rev().find_map(|event| match *event {
        CompositorEvent::Scroll {
            node,
            offset,
            serial,
            ..
        } if node == list.scroll => Some((serial, offset)),
        _ => None,
    });
    let Some((serial, offset)) = newest else {
        return;
    };
    if cadence.jump.is_some_and(|jump| frame.is_multiple_of(jump)) {
        tx.scroll_to(list.scroll, [offset[0], offset[1] + 2.0]);
    } else if frame.is_multiple_of(cadence.acknowledge) {
        tx.acknowledge_scroll(list.scroll, serial, offset);
    }
}

/// Returns a quad with square corners and no border over `bounds`, filled with `fill`.
fn quad_of(bounds: [f32; 4], fill: PaintRef) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}
