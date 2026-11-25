//! The random edits of a scene.

use iceblink::{ClipNode, ClipShape, Matrix4, OrderEntry, Paint, ROUND, Transaction, TransformId};

use crate::conformance::end_to_end::numbers::Numbers;
use crate::conformance::end_to_end::scene::items::{colour, effect, record};
use crate::conformance::end_to_end::scene::{CHUNKS, Ids, Model, NODES, key, node_for};

/// Takes chunk `part` out of the node that holds it, if one does.
fn take(tx: &mut Transaction, ids: &Ids, model: &mut Model, part: u32) {
    if let Some((node, at)) = model.place(part) {
        model.held[node].remove(at);
        tx.splice(ids.nodes[node], at as u32, 1, &[]);
    }
}

/// Puts chunk `part`, which no node holds, at a random place in a random node.
fn put(tx: &mut Transaction, ids: &Ids, model: &mut Model, part: u32) {
    let node = node_for(&mut model.numbers, part);
    let at = model.numbers.below(model.held[node].len() as u64 + 1) as usize;
    model.held[node].insert(at, part);
    tx.splice(
        ids.nodes[node],
        at as u32,
        0,
        &[OrderEntry::Chunk(key(part))],
    );
}

/// Returns a random port clip near `PORT`, with round corners half of the time.
fn port(numbers: &mut Numbers, space: TransformId) -> ClipNode {
    let rect = [
        numbers.quarter(2.0, 24),
        numbers.quarter(2.0, 24),
        numbers.quarter(56.0, 64),
        numbers.quarter(100.0, 96),
    ];
    let mut radii = [0.0; 8];
    if numbers.below(2) == 0 {
        for corner in radii.as_chunks_mut::<2>().0 {
            corner.fill(numbers.quarter(0.0, 40));
        }
    }
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii,
            shapes: ROUND,
        },
    }
}

/// Writes one random edit: a scroll, a move of the free space, an effect, a paint, a splice, a
/// removal, a recording, a chunk offset or a port clip.
pub(crate) fn edit(tx: &mut Transaction, ids: &Ids, model: &mut Model) {
    let part = model.numbers.below(u64::from(CHUNKS)) as u32;
    let blend_modes = model.blend_modes;
    let numbers = &mut model.numbers;
    match numbers.below(11) {
        0 | 1 => {
            let half = if numbers.below(4) == 0 { 0.5 } else { 0.0 };
            let by = numbers.below(41) as f32 - 20.0 + half;
            model.offset = (model.offset + by).clamp(0.0, 300.0);
            tx.scroll_to(ids.scroll, [0.0, model.offset]);
        }
        2 => {
            let x = numbers.quarter(-12.0, 96);
            let y = numbers.quarter(-12.0, 96);
            tx.set_transform(ids.free, Matrix4::translation(x, y, 0.0));
        }
        3 => {
            let node = 1 + numbers.below(NODES as u64 - 1) as usize;
            tx.set_effect(
                ids.effects[node - 1],
                effect(numbers, node, ids.port, blend_modes),
            );
        }
        4 => {
            let paint = ids.paints[numbers.below(3) as usize];
            tx.set_paint(paint, Paint::Solid(colour(numbers)));
        }
        5 => {
            take(tx, ids, model, part);
            put(tx, ids, model, part);
        }
        6 => take(tx, ids, model, part),
        7 => record(tx, ids, numbers, part),
        8 => {
            let offset = [numbers.below(9) as f32 - 4.0, numbers.below(9) as f32 - 4.0];
            tx.set_offset(key(part), offset);
        }
        9 => tx.set_clip(ids.port, port(numbers, ids.space)),
        _ => {
            if model.place(part).is_none() {
                put(tx, ids, model, part);
            }
        }
    }
}
