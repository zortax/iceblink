//! The driver keeps a fling moving at the display's rate while the client's thread is blocked.

mod common;

use iceblink::{ClientEvent, CompositorClient};
use iceblink::{CompositorEvent, FrameOutcome, ScrollPhase};

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, scrolls, spawn};
use crate::common::thread::watched::Watched;

/// How many frames the client stays blocked for.
const BLOCKED_FRAMES: usize = 40;

#[test]
fn a_blocked_client_thread_does_not_stall_a_fling() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let mut events = Vec::new();
    handle.drain_events(list.client, &mut events);
    list.fling(&mut handle, 3000.0);
    let start = probe.draws();

    // The client's thread blocks on something else while the presenter's refreshes pass.
    let draws = probe.wait_draws(start + BLOCKED_FRAMES);
    let fling = &draws[start..start + BLOCKED_FRAMES];

    for pair in fling.windows(2) {
        let [before, after] = pair else {
            unreachable!()
        };
        assert!(after.clock.present_ns > before.clock.present_ns);
        // At most one frame starts per refresh of the display, however late it starts.
        assert!(
            after.clock.now_ns / PERIOD_NS > before.clock.now_ns / PERIOD_NS,
            "{before:?} and {after:?} start in one refresh"
        );
        assert_eq!(after.clock.interval_ns, PERIOD_NS);
    }
    assert!(
        fling
            .iter()
            .all(|drawn| matches!(drawn.outcome, FrameOutcome::Presented(_)))
    );

    handle.drain_events(list.client, &mut events);
    let moves = scrolls(&events, list.node);
    let momentum: Vec<f32> = moves
        .iter()
        .filter(|(_, _, phase)| *phase == ScrollPhase::Momentum)
        .map(|(_, offset, _)| *offset)
        .collect();
    assert!(
        matches!(momentum[..], [offset] if offset > 10.0),
        "the newest event of the motion is queued, and the fling moved the list: {momentum:?}"
    );
    assert!(
        !events.iter().any(|event| matches!(
            event,
            ClientEvent::Compositor(CompositorEvent::Scroll {
                phase: ScrollPhase::Settled,
                ..
            })
        )),
        "the fling is still moving"
    );

    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}
