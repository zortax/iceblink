//! The scene the random edits start from.

use iceblink::{
    BorderStyle, ChunkKey, ClipNode, ClipRef, ClipShape, Color, EffectId, OrderEntry, OrderNode,
    OutputId, Paint, PaintKind, PaintRef, Quad, ROUND, ScrollNode, SpaceRef, Transaction,
    TransformNode,
};

use crate::conformance::end_to_end::scene::items::{colour, effect, record};
use crate::conformance::end_to_end::scene::{
    CHUNKS, Ids, Model, NODES, OUTER, PORT, identity, key, node_for, props,
};

/// Builds a scene of an opaque backing, a scroll port with its content, the five order nodes and
/// the chunks placed among them, and returns its ids.
pub(crate) fn scene(tx: &mut Transaction, output: OutputId, model: &mut Model) -> Ids {
    let blend_modes = model.blend_modes;
    let numbers = &mut model.numbers;
    let space = tx.create_transform(identity(None));
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    let white = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
    let mut backing = tx.record(ChunkKey::new(1, 0), props(space, None));
    let fill = backing.paint_ref(PaintKind::SOLID, white);
    backing.quad(Quad {
        bounds: [0.0, 0.0, 128.0, 128.0],
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
    });
    tx.splice(root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);

    let scroll = tx.create_scroll(ScrollNode {
        space,
        port: PORT,
        content: [100.0, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(space))
    });
    let free = tx.create_transform(identity(Some(space)));
    let port = tx.create_clip(ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: PORT,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let paints = [(); 3].map(|_| tx.create_paint(Paint::Solid(colour(numbers))));

    let list = tx.create_order(OrderNode::default());
    let mut nodes = [list; NODES];
    let mut effects = Vec::new();
    for (node, slot) in nodes.iter_mut().enumerate().skip(1) {
        let id = tx.create_effect(effect(numbers, node, port, blend_modes));
        effects.push(id);
        *slot = tx.create_order(OrderNode { effect: Some(id) });
    }
    let effects: [EffectId; NODES - 1] = effects.try_into().expect("one effect per group");
    let placed: Vec<OrderEntry> = nodes[..4]
        .iter()
        .map(|&node| OrderEntry::Node(node))
        .collect();
    tx.splice(root, 1, 0, &placed);

    let ids = Ids {
        space,
        scroll,
        free,
        port,
        paints,
        nodes,
        effects,
        spaces: [content, free, space],
    };
    for part in 0..CHUNKS {
        record(tx, &ids, numbers, part);
        let node = node_for(numbers, part);
        model.held[node].push(part);
    }
    for (node, held) in model.held.iter().enumerate() {
        let entries: Vec<OrderEntry> = held
            .iter()
            .map(|&part| OrderEntry::Chunk(key(part)))
            .collect();
        tx.splice(ids.nodes[node], 0, 0, &entries);
    }
    let nested = OrderEntry::Node(nodes[4]);
    tx.splice(nodes[OUTER], model.held[OUTER].len() as u32, 0, &[nested]);
    ids
}
