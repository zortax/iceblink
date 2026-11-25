//! A paced driver stamps gestures with its frame clock.

mod common;

use iceblink::{ClientEvent, CompositorClient};
use iceblink::{GesturePhase, ScrollGesture, ScrollRouting, ScrollSource};
use iceblink_driver::now_ns;

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, spawn};
use crate::common::thread::watched::Watched;

#[test]
fn a_paced_driver_stamps_a_gesture_when_it_is_handed() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let mut events = Vec::new();
    handle.drain_events(list.client, &mut events);

    let before = now_ns();
    let gesture = ScrollGesture {
        position: [100.0, 100.0],
        delta: [0.0, 10.0],
        phase: GesturePhase::Begin,
        source: ScrollSource::Touchpad,
        time_ns: 0,
    };
    handle.scroll_input(list.client, list.output, gesture, ScrollRouting::Unhandled);
    let after = now_ns();
    handle.sync();

    events.clear();
    handle.drain_events(list.client, &mut events);
    let stamped = events.iter().find_map(|event| match event {
        ClientEvent::Rerouted { gesture, .. } => Some(gesture.time_ns),
        _ => None,
    });
    assert!(
        stamped.is_some_and(|stamp| (before..=after).contains(&stamp)),
        "the gesture is stamped between {before} and {after}, found {stamped:?}"
    );
    driver.shutdown();
}
