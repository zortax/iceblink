//! Rules of routing gestures by the scroll port under the position.

use iceblink::{
    ChunkKey, ChunkProps, ClipNode, ClipShape, GesturePhase, OrderEntry, ROUND, ScrollId,
    ScrollPhysics, Sources, Transaction, TransformId,
};

use crate::conformance::tier::scrollers::{Scroller, handled, scroller, vertical};
use crate::conformance::tier::stage::{Factory, Stage, props, quads};

/// Creates a scroll node over `port` in `space` with content 1000 tall that scrolls vertically
/// with instant physics and chains, and returns it with the transform node of its content.
fn port(tx: &mut Transaction, space: TransformId, port: [f32; 4]) -> (ScrollId, TransformId) {
    let behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::ALL);
    let Scroller { node, content } = scroller(tx, space, port, 1000.0, Some(behaviour));
    (node, content)
}

/// Returns the node a wheel step down at `at` was routed to.
fn routed(stage: &mut Stage, at: [f32; 2]) -> Option<ScrollId> {
    handled(stage.gesture(at, [0.0, 10.0], GesturePhase::Step, 0))
}

/// A gesture over a part of a port its content leaves blank routes to the port, whether nothing
/// is drawn there or a backdrop is drawn behind the port.
pub fn a_blank_part_of_a_port_routes_to_the_port(factory: Factory<'_>) {
    for backdrop in [false, true] {
        let mut stage = Stage::new(factory);
        let mut tx = stage.tx();
        if backdrop {
            let key = ChunkKey::new(1, 0);
            quads(
                &mut tx,
                key,
                props(stage.space),
                &[[0.0, 0.0, 200.0, 200.0]],
            );
            stage.push(&mut tx, OrderEntry::Chunk(key));
        }
        let (node, content) = port(&mut tx, stage.space, [20.0, 20.0, 160.0, 160.0]);
        let key = ChunkKey::new(2, 0);
        quads(&mut tx, key, props(content), &[[20.0, 20.0, 20.0, 20.0]]);
        stage.push(&mut tx, OrderEntry::Chunk(key));
        stage.commit(tx);
        stage.draw_at(0);
        assert_eq!(
            routed(&mut stage, [100.0, 100.0]),
            Some(node),
            "a blank part of the port takes the gesture, with a backdrop: {backdrop}"
        );
    }
}

/// A gesture inside a port whose content draws nothing routes to the port, whether nothing is
/// drawn there or a backdrop is drawn behind the port, and a gesture over another port's content
/// drawn there routes to that port.
pub fn a_port_whose_content_draws_nothing_routes_to_the_port(factory: Factory<'_>) {
    for backdrop in [false, true] {
        let mut stage = Stage::new(factory);
        let mut tx = stage.tx();
        if backdrop {
            let key = ChunkKey::new(1, 0);
            quads(
                &mut tx,
                key,
                props(stage.space),
                &[[0.0, 0.0, 200.0, 200.0]],
            );
            stage.push(&mut tx, OrderEntry::Chunk(key));
        }
        let (node, _) = port(&mut tx, stage.space, [20.0, 20.0, 160.0, 160.0]);
        stage.commit(tx);
        stage.draw_at(0);
        assert_eq!(
            routed(&mut stage, [100.0, 100.0]),
            Some(node),
            "the port takes the gesture, with a backdrop: {backdrop}"
        );
    }

    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    port(&mut tx, stage.space, [0.0, 0.0, 200.0, 200.0]);
    let (list, content) = port(&mut tx, stage.space, [100.0, 100.0, 80.0, 80.0]);
    let key = ChunkKey::new(1, 0);
    quads(
        &mut tx,
        key,
        props(content),
        &[[100.0, 100.0, 80.0, 1000.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(key));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(
        routed(&mut stage, [140.0, 140.0]),
        Some(list),
        "a list beside the empty port keeps the gestures over its rows"
    );
}

/// A gesture over something drawn over a port, and not in its content, does not route to the
/// port.
pub fn what_is_drawn_over_a_port_keeps_its_gestures(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let (_, content) = port(&mut tx, stage.space, [20.0, 20.0, 160.0, 160.0]);
    let (rows, overlay) = (ChunkKey::new(1, 0), ChunkKey::new(2, 0));
    quads(
        &mut tx,
        rows,
        props(content),
        &[[20.0, 20.0, 160.0, 1000.0]],
    );
    quads(
        &mut tx,
        overlay,
        props(stage.space),
        &[[60.0, 60.0, 80.0, 80.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(rows));
    stage.push(&mut tx, OrderEntry::Chunk(overlay));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(
        routed(&mut stage, [100.0, 100.0]),
        None,
        "an overlay drawn over the port keeps the gesture"
    );
}

/// A gesture over a blank part of a port nested in another port's content routes to the inner
/// port, and over an outer item drawn over the inner port to the outer port.
pub fn a_blank_part_of_a_nested_port_routes_to_the_inner_port(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let (outer, outer_content) = port(&mut tx, stage.space, [0.0, 0.0, 200.0, 200.0]);
    let (inner, inner_content) = port(&mut tx, outer_content, [20.0, 20.0, 160.0, 100.0]);
    let (page, rows, over) = (
        ChunkKey::new(1, 0),
        ChunkKey::new(2, 0),
        ChunkKey::new(3, 0),
    );
    quads(
        &mut tx,
        page,
        props(outer_content),
        &[[0.0, 0.0, 200.0, 1000.0]],
    );
    quads(
        &mut tx,
        rows,
        props(inner_content),
        &[[20.0, 20.0, 20.0, 20.0]],
    );
    quads(
        &mut tx,
        over,
        props(outer_content),
        &[[120.0, 80.0, 40.0, 30.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(page));
    stage.push(&mut tx, OrderEntry::Chunk(rows));
    stage.push(&mut tx, OrderEntry::Chunk(over));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(
        routed(&mut stage, [80.0, 60.0]),
        Some(inner),
        "a blank part of the inner port takes the gesture before the outer port"
    );
    assert_eq!(
        routed(&mut stage, [140.0, 90.0]),
        Some(outer),
        "an outer item drawn over the inner port routes to the outer port"
    );
}

/// A gesture outside an outer port does not route to the part of an inner port that lies past
/// the outer port's edge.
pub fn a_port_counts_only_inside_the_ports_around_it(factory: Factory<'_>) {
    let mut stage = Stage::new(factory);
    let mut tx = stage.tx();
    let (outer, outer_content) = port(&mut tx, stage.space, [0.0, 0.0, 200.0, 150.0]);
    let (_, inner_content) = port(&mut tx, outer_content, [20.0, 100.0, 160.0, 100.0]);
    let (toolbar, page, rows) = (
        ChunkKey::new(1, 0),
        ChunkKey::new(2, 0),
        ChunkKey::new(3, 0),
    );
    let outer_clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 200.0, 150.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let inner_clip = tx.create_clip(ClipNode {
        parent: Some(outer_clip),
        space: outer_content,
        shape: ClipShape::RoundedRect {
            rect: [20.0, 100.0, 160.0, 100.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let clipped = |space, clip| ChunkProps {
        clip: Some(clip),
        ..props(space)
    };
    quads(
        &mut tx,
        toolbar,
        props(stage.space),
        &[[0.0, 150.0, 200.0, 50.0]],
    );
    quads(
        &mut tx,
        page,
        clipped(outer_content, outer_clip),
        &[[0.0, 0.0, 200.0, 1000.0]],
    );
    quads(
        &mut tx,
        rows,
        clipped(inner_content, inner_clip),
        &[[20.0, 100.0, 160.0, 1000.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(toolbar));
    stage.push(&mut tx, OrderEntry::Chunk(page));
    stage.push(&mut tx, OrderEntry::Chunk(rows));
    stage.commit(tx);
    stage.draw_at(0);
    assert_eq!(
        routed(&mut stage, [100.0, 175.0]),
        None,
        "the toolbar below the outer port keeps the gesture"
    );
    assert_eq!(
        routed(&mut stage, [100.0, 50.0]),
        Some(outer),
        "the outer port takes a gesture over its own content"
    );
}
