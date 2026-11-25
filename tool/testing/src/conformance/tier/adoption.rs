//! Rules of acknowledging compositor scrolling at the device pixels it was shown at.

use iceblink::{
    Anchor, Built, ChunkKey, GesturePhase, Matrix4, OrderEntry, ScrollId, ScrollNode,
    ScrollPhysics, Sources, TransformNode,
};

use crate::conformance::tier::FRAME_NS;
use crate::conformance::tier::scrollers::{frames, vertical};
use crate::conformance::tier::stage::{Factory, Stage, props, quads};

const MS: u64 = 1_000_000;

/// Acknowledging the newest event of a delta that ended half a device pixel past a whole one
/// changes no pixel, at any scale of the node's space and in either direction.
pub fn acknowledging_a_half_pixel_moves_no_pixel(factory: Factory<'_>) {
    for scale in [1.0, 1.25, 1.5] {
        for delta in [2.5, 4.5, 7.5, -2.5, -4.5, -7.5] {
            let (mut stage, node) = scaled(factory, scale);
            stage.gesture([50.0, 50.0], [0.0, 0.0], GesturePhase::Begin, 0);
            stage.gesture([50.0, 50.0], [0.0, delta], GesturePhase::Update, MS);
            frames(&mut stage, FRAME_NS, FRAME_NS);
            stage.gesture(
                [50.0, 50.0],
                [0.0, 0.0],
                GesturePhase::End {
                    velocity: [0.0, 0.0],
                },
                2 * FRAME_NS,
            );
            let events = frames(&mut stage, 2 * FRAME_NS, 4 * FRAME_NS);
            let (offset, _, serial, _) =
                Stage::last_scroll(&events, node).expect("the gesture moved the node");

            let mut tx = stage.tx();
            tx.acknowledge_scroll(node, serial, offset);
            stage.commit(tx);
            assert_eq!(
                stage.draw_at(5 * FRAME_NS),
                Built::Idle { wake_ns: None },
                "acknowledging {offset:?} after a delta of {delta} pixels at scale {scale} \
                 changes no pixel"
            );
        }
    }
}

/// A stage with a port at `scale` device pixels per unit over the top left of the output, onto
/// content 2000 tall, scrolled 100 down.
fn scaled(factory: Factory<'_>, scale: f32) -> (Stage, ScrollId) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let space = tx.create_transform(TransformNode {
        parent: Some(stage.space),
        local: Matrix4::scale(scale, scale, 1.0),
        scroll: None,
        anchor: Anchor::Scrolls,
    });
    let behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::NONE);
    let node = tx.create_scroll(ScrollNode {
        space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [100.0, 2000.0],
        offset: [0.0, 100.0],
        behaviour: Some(behaviour),
    });
    let content = tx.create_transform(TransformNode {
        parent: Some(space),
        local: Matrix4::IDENTITY,
        scroll: Some(node),
        anchor: Anchor::Scrolls,
    });
    let rows: Vec<[f32; 4]> = (0..100)
        .map(|row| [0.0, row as f32 * 20.0, 100.0, 20.0])
        .collect();
    quads(&mut tx, ChunkKey::new(1, 0), props(content), &rows);
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.commit(tx);
    stage.draw_at(0);
    (stage, node)
}
