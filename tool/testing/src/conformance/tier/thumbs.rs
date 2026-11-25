//! Rules of thumb nodes under compositor scrolling.

use iceblink::{
    Anchor, ChunkKey, GesturePhase, OrderEntry, ScrollNode, ScrollPhysics, Sources, ThumbAnchor,
    TransformNode,
};

use crate::conformance::tier::FRAME_NS;
use crate::conformance::tier::scrollers::vertical;
use crate::conformance::tier::stage::{Factory, Stage, identity, props, quads};

/// Reports whether the frontmost hit at `point` is an item of `key`.
fn hits(stage: &Stage, point: [f32; 2], key: ChunkKey) -> bool {
    stage
        .compositor
        .hit_test(stage.output, point)
        .first()
        .is_some_and(|hit| hit.chunk == key)
}

/// A thumb node follows the offset the compositor gives its scroll node, with no client write.
pub fn a_thumb_follows_compositor_scrolling(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let node = tx.create_scroll(ScrollNode {
        space: stage.space,
        port: [0.0, 0.0, 180.0, 200.0],
        content: [180.0, 2000.0],
        offset: [0.0; 2],
        behaviour: Some(vertical(
            ScrollPhysics::Instant,
            Sources::NONE,
            Sources::NONE,
        )),
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(node),
        ..identity(Some(stage.space))
    });
    let thumb = tx.create_transform(TransformNode {
        anchor: Anchor::Thumb(ThumbAnchor {
            scroll: node,
            track: [0.0, 160.0],
        }),
        ..identity(Some(stage.space))
    });
    let (list, bar) = (ChunkKey::new(1, 0), ChunkKey::new(2, 0));
    quads(&mut tx, list, props(content), &[[0.0, 0.0, 180.0, 2000.0]]);
    quads(&mut tx, bar, props(thumb), &[[186.0, 0.0, 8.0, 40.0]]);
    stage.push(&mut tx, OrderEntry::Chunk(list));
    stage.push(&mut tx, OrderEntry::Chunk(bar));
    stage.commit(tx);
    stage.draw_at(0);
    assert!(
        hits(&stage, [190.0, 20.0], bar),
        "the thumb starts at the top"
    );

    // A tenth of the range moves the thumb a tenth of its track.
    stage.gesture([90.0, 100.0], [0.0, 180.0], GesturePhase::Step, 0);
    stage.draw_at(FRAME_NS);
    assert!(
        !hits(&stage, [190.0, 10.0], bar),
        "the thumb left the top of its track"
    );
    assert!(
        hits(&stage, [190.0, 50.0], bar),
        "the thumb moved by its share of the offset"
    );
    assert!(
        !hits(&stage, [190.0, 60.0], bar),
        "the thumb moved by no more than its share of the offset"
    );
}
