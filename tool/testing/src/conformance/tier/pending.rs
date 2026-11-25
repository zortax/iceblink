//! Rules of gesture steps that wait for the next frame while the client commits.

use iceblink::{GesturePhase, ScrollNode, ScrollPhysics, Sources};

use crate::conformance::tier::FRAME_NS;
use crate::conformance::tier::scrollers::{list, vertical};
use crate::conformance::tier::stage::{Factory, Stage};

/// An update handed before a commit that acknowledges the newest offset shows in the next frame,
/// which emits a scroll event with the offset it shows.
pub fn an_update_waiting_for_an_adoption_shows_with_its_event(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    stage.gesture([100.0, 100.0], [0.0, 0.0], GesturePhase::Begin, 0);
    stage.gesture([100.0, 100.0], [0.0, 10.0], GesturePhase::Update, 0);
    stage.draw_at(FRAME_NS);
    let events = stage.events();
    let (offset, _, serial, _) = Stage::last_scroll(&events, node).expect("the update moved");

    stage.gesture([100.0, 100.0], [0.0, 10.0], GesturePhase::Update, FRAME_NS);
    let mut tx = stage.tx();
    tx.acknowledge_scroll(node, serial, offset);
    stage.commit(tx);
    stage.draw_at(2 * FRAME_NS);
    let events = stage.events();
    let shown = Stage::last_scroll(&events, node).map(|(offset, ..)| offset[1]);
    assert_eq!(shown, Some(20.0), "{events:?}");
}

/// A node write at the base offset while an update waits for the next frame leaves the offset
/// shown until that frame, which shows the update.
pub fn a_node_write_waiting_for_its_frame_keeps_the_shown_offset(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour.clone()));
    stage.gesture([100.0, 100.0], [0.0, 0.0], GesturePhase::Begin, 0);
    stage.gesture([100.0, 100.0], [0.0, 10.0], GesturePhase::Update, 0);
    stage.draw_at(FRAME_NS);
    stage.events();

    stage.gesture([100.0, 100.0], [0.0, 10.0], GesturePhase::Update, FRAME_NS);
    let mut tx = stage.tx();
    tx.set_scroll_node(
        node,
        ScrollNode {
            space: stage.space,
            port: [0.0, 0.0, 200.0, 200.0],
            content: [200.0, 4000.0],
            offset: [0.0; 2],
            behaviour: Some(behaviour),
        },
    );
    stage.commit(tx);
    // Row 0 spans 0 to 20, and holds the point at an offset of 10 and not at 20.
    let hits = stage.compositor.hit_test(stage.output, [100.0, 5.0]);
    assert_eq!(hits.first().map(|hit| hit.item), Some(0), "{hits:?}");

    stage.draw_at(2 * FRAME_NS);
    let events = stage.events();
    let shown = Stage::last_scroll(&events, node).map(|(offset, ..)| offset[1]);
    assert_eq!(shown, Some(20.0), "{events:?}");
}
