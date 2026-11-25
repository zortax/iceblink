//! Rules of latching a gesture onto a node and passing the rest of its delta on.

use iceblink::{
    ChunkFlags, ChunkKey, GesturePhase, OrderEntry, ScrollPhysics, ScrollRouting, Sources,
};

use crate::conformance::tier::FRAME_NS;
use crate::conformance::tier::scrollers::{
    fling_end, frames, handled, list, nested, offset_of, vertical,
};
use crate::conformance::tier::stage::{Factory, Stage, props, quads};

const MS: u64 = 1_000_000;

/// A gesture latches onto the innermost scroll node under its position that can move by its
/// delta.
pub fn a_gesture_latches_onto_the_innermost_node_that_can_move(factory: Factory<'_>) {
    let inner_behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::ALL);
    let (mut stage, outer, inner) = nested(factory, inner_behaviour);
    let begin = |stage: &mut Stage, at: [f32; 2], dy: f32| {
        handled(stage.gesture(at, [0.0, dy], GesturePhase::Begin, 0))
    };
    // Upwards neither node can move from the top.
    assert_eq!(begin(&mut stage, [100.0, 50.0], -10.0), None);
    assert_eq!(begin(&mut stage, [100.0, 150.0], 10.0), Some(outer));
    assert_eq!(begin(&mut stage, [100.0, 50.0], 10.0), Some(inner));

    let mut tx = stage.tx();
    tx.scroll_to(inner, [0.0, 300.0]);
    stage.commit(tx);
    assert_eq!(begin(&mut stage, [100.0, 50.0], 10.0), Some(outer));
}

/// On each axis the latched node absorbs what fits in its range, and a chaining node passes the
/// rest outwards.
pub fn the_rest_of_a_delta_chains_outwards(factory: Factory<'_>) {
    let inner_behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::ALL);
    let (mut stage, outer, inner) = nested(factory, inner_behaviour);
    let mut tx = stage.tx();
    tx.scroll_to(inner, [0.0, 290.0]);
    stage.commit(tx);
    stage.gesture([100.0, 50.0], [0.0, 30.0], GesturePhase::Begin, 0);
    let events = frames(&mut stage, FRAME_NS, FRAME_NS);
    assert_eq!(offset_of(&events, inner), 300.0);
    assert_eq!(offset_of(&events, outer), 20.0);
}

/// The rest of a delta that a node that does not chain cannot absorb is dropped by a node that
/// does not bounce, and displaces the content of one that does by `e (1 - 1 / (0.55 x / e + 1))`.
pub fn the_rest_past_the_end_clamps_or_bounces(factory: Factory<'_>) {
    for (bounce, shown) in [
        (Sources::NONE, 0.0),
        (
            Sources::ALL,
            -(100.0 * (1.0 - 1.0 / (0.55 * 40.0 / 100.0 + 1.0))),
        ),
    ] {
        let inner_behaviour = vertical(ScrollPhysics::Instant, bounce, Sources::NONE);
        let (mut stage, outer, inner) = nested(factory, inner_behaviour);
        stage.gesture([100.0, 50.0], [0.0, 0.0], GesturePhase::Begin, 0);
        assert_eq!(
            handled(stage.gesture([100.0, 50.0], [0.0, -40.0], GesturePhase::Update, MS)),
            if bounce == Sources::ALL {
                Some(inner)
            } else {
                None
            },
        );
        let events = frames(&mut stage, FRAME_NS, FRAME_NS);
        let moved = Stage::last_scroll(&events, inner).map_or(0.0, |scroll| scroll.1[1]);
        assert!((moved - shown).abs() <= 0.5, "{moved} is not {shown}");
        assert!(Stage::last_scroll(&events, outer).is_none());
    }
}

/// A gesture is unhandled when no node under its position is user-scrollable on an axis of its
/// delta, and hit-transparent chunks do not take it.
pub fn gestures_need_a_node_that_scrolls(factory: Factory<'_>) {
    let (mut stage, _) = list(factory, None);
    let routing = stage.gesture([100.0, 100.0], [0.0, 10.0], GesturePhase::Begin, 0);
    assert_eq!(routing, ScrollRouting::Unhandled);

    let behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::NONE);
    let (mut stage, _) = list(factory, Some(behaviour.clone()));
    let routing = stage.gesture([100.0, 100.0], [10.0, 0.0], GesturePhase::Begin, 0);
    assert_eq!(routing, ScrollRouting::Unhandled);

    let (mut stage, node) = list(factory, Some(behaviour));
    let mut tx = stage.tx();
    let mut transparent = props(stage.space);
    transparent.flags = ChunkFlags::HIT_TRANSPARENT;
    quads(
        &mut tx,
        ChunkKey::new(9, 0),
        transparent,
        &[[0.0, 0.0, 200.0, 200.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(9, 0)));
    stage.commit(tx);
    let routing = stage.gesture([100.0, 100.0], [0.0, 10.0], GesturePhase::Begin, 0);
    assert_eq!(handled(routing), Some(node));
}

/// A release that finds the gesture latched onto no node is routed by its velocity, and flings.
pub fn an_unlatched_release_routes_by_its_velocity(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    stage.gesture([100.0, 100.0], [0.0, 0.0], GesturePhase::Begin, 0);
    let routing = stage.gesture(
        [100.0, 100.0],
        [0.0, 0.0],
        GesturePhase::End {
            velocity: [0.0, 1000.0],
        },
        0,
    );
    assert_eq!(handled(routing), Some(node));
    let events = frames(&mut stage, FRAME_NS, 3_000 * MS);
    let rest = fling_end(1000.0, FRAME_NS).0;
    assert!((f64::from(offset_of(&events, node)) - rest).abs() <= 1.0);
}
