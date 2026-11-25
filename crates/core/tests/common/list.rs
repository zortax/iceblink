//! A scrolled list of rows over an opaque backing.

use std::sync::Arc;

use iceblink::{
    BlendMode, ChunkKey, ClipId, ClipNode, ClipShape, EffectNode, OrderEntry, OrderId, OrderNode,
    PaintId, PaintKind, ROUND, ScrollId, ScrollNode, Transaction, TransformId, TransformNode,
};
use iceblink::{Compositor, Renderer};

use crate::common::slice::Slice;
use crate::common::{identity, props, quad};

/// The objects [`list`] records for a scrolled list.
pub(crate) struct List {
    pub(crate) scroll: ScrollId,
    /// The order node holding the rows.
    pub(crate) node: OrderId,
    content: TransformId,
    port: ClipId,
    paint: PaintId,
}

impl List {
    /// Records row `row` of the list in `tx`, and returns its chunk.
    pub(crate) fn row(&self, tx: &mut Transaction, row: u32) -> ChunkKey {
        let key = ChunkKey::new(2, row);
        let mut row_props = props(self.content);
        row_props.clip = Some(self.port);
        let mut chunk = tx.record(key, row_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, self.paint);
        chunk.quad(quad([4.0, row as f32 * 10.0 + 2.0, 80.0, 6.0], fill));
        key
    }
}

/// Commits a scrolled list of `rows` rows over an opaque backing quad, and draws a frame.
///
/// Each row is 10 pixels tall and a chunk of its own. The rows share an order node inside a clipped
/// scroll port of 100 by 100. With `faded`, every fourth row is drawn at half opacity in a group
/// clipped by the port.
pub(crate) fn list<R: Renderer>(slice: &mut Slice<R>, rows: u32, faded: bool) -> List {
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let backing = world.solid(&mut tx, [1.0, 1.0, 1.0, 1.0]);
    world.boxes(
        &mut tx,
        ChunkKey::new(1, 0),
        backing,
        &[[0.0, 0.0, 128.0, 128.0]],
        true,
    );

    let scroll = tx.create_scroll(ScrollNode {
        space: world.space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [100.0, rows as f32 * 10.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(world.space))
    });
    let port = tx.create_clip(ClipNode {
        parent: None,
        space: world.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 100.0, 100.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });

    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    let fade = tx.create_effect(EffectNode {
        clip: Some(port),
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from(Vec::new()),
        isolate: false,
    });

    let list = tx.create_order(OrderNode::default());
    let mut entries = Vec::new();
    let made = List {
        scroll,
        node: list,
        content,
        port,
        paint,
    };
    for row in 0..rows {
        let key = made.row(&mut tx, row);
        if faded && row % 4 == 0 {
            let group = tx.create_order(OrderNode { effect: Some(fade) });
            tx.splice(group, 0, 0, &[OrderEntry::Chunk(key)]);
            entries.push(OrderEntry::Node(group));
        } else {
            entries.push(OrderEntry::Chunk(key));
        }
    }

    tx.splice(list, 0, 0, &entries);
    world.append(&mut tx, OrderEntry::Node(list));
    slice.compositor.commit(tx).expect("the list commits");
    assert!(slice.draw());
    made
}
