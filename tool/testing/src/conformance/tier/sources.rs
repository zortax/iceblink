//! Rules of scroll behaviour that differs between the sources of gestures.

use iceblink::{GesturePhase, ScrollPhysics, Sources};

use crate::conformance::tier::FRAME_NS;
use crate::conformance::tier::scrollers::{frames, handled, nested, vertical};
use crate::conformance::tier::stage::{Factory, Stage};

const MS: u64 = 1_000_000;

/// A delta past the end bounces only for the sources the behaviour's `bounce` lists, and is
/// dropped for the others.
pub fn overscroll_bounces_only_the_sources_that_bounce(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Instant, Sources::TOUCHPAD, Sources::NONE);
    let (mut stage, _, inner) = nested(factory, behaviour);

    let routed = handled(stage.gesture([100.0, 50.0], [0.0, -40.0], GesturePhase::Step, 0));
    let events = frames(&mut stage, FRAME_NS, FRAME_NS);
    assert_eq!(
        routed, None,
        "a wheel step past the end has no node to bounce"
    );
    assert!(
        Stage::last_scroll(&events, inner).is_none(),
        "a wheel step past the end is dropped"
    );

    stage.gesture([100.0, 50.0], [0.0, 0.0], GesturePhase::Begin, 2 * FRAME_NS);
    let routed = handled(stage.gesture(
        [100.0, 50.0],
        [0.0, -40.0],
        GesturePhase::Update,
        2 * FRAME_NS + MS,
    ));
    let events = frames(&mut stage, 3 * FRAME_NS, 3 * FRAME_NS);
    assert_eq!(routed, Some(inner), "a touchpad pull past the end latches");
    let shown = Stage::last_scroll(&events, inner).map_or(0.0, |scroll| scroll.1[1]);
    assert!(
        shown < 0.0,
        "a touchpad pull past the end bounces, by {shown}"
    );
}

/// The rest of a delta passes outwards only for the sources the behaviour's `chain` lists.
pub fn the_rest_chains_only_for_the_sources_that_chain(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::WHEEL);
    let (mut stage, outer, inner) = nested(factory, behaviour);
    let mut tx = stage.tx();
    tx.scroll_to(inner, [0.0, 290.0]);
    stage.commit(tx);

    stage.gesture([100.0, 50.0], [0.0, 0.0], GesturePhase::Begin, 0);
    stage.gesture([100.0, 50.0], [0.0, 30.0], GesturePhase::Update, MS);
    stage.gesture(
        [100.0, 50.0],
        [0.0, 0.0],
        GesturePhase::End {
            velocity: [0.0, 0.0],
        },
        2 * MS,
    );
    let events = frames(&mut stage, FRAME_NS, 2 * FRAME_NS);
    assert_eq!(
        Stage::last_scroll(&events, inner).map(|scroll| scroll.0[1]),
        Some(300.0)
    );
    assert!(
        Stage::last_scroll(&events, outer).is_none(),
        "a touchpad gesture stays on the node it latched onto"
    );

    let mut tx = stage.tx();
    tx.scroll_to(inner, [0.0, 290.0]);
    stage.commit(tx);
    stage.gesture([100.0, 50.0], [0.0, 30.0], GesturePhase::Step, 3 * FRAME_NS);
    let events = frames(&mut stage, 4 * FRAME_NS, 4 * FRAME_NS);
    assert_eq!(
        Stage::last_scroll(&events, outer).map(|scroll| scroll.0[1]),
        Some(20.0),
        "a wheel step passes its rest outwards"
    );
}
