//! Events are queued for the client in the order they happened, and wake it through its waker.

mod common;

use std::sync::Arc;

use iceblink::{AnimationEvent, BlendMode, ClientId, CompositorEvent, EffectNode};
use iceblink::{ClientEvent, CompositorClient};
use iceblink_driver::thread::ClientHandle;

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, commit, fade, spawn};
use crate::common::thread::wake::Wakes;
use crate::common::thread::watched::Watched;

/// Drains events each time the waker runs, until `done` holds for the events drained so far.
fn wake_until(
    handle: &mut ClientHandle,
    client: ClientId,
    wakes: &Wakes,
    mut done: impl FnMut(&[ClientEvent]) -> bool,
) -> Vec<ClientEvent> {
    let mut events = Vec::new();
    while !done(&events) {
        let thread = wakes.next();
        assert_eq!(
            thread.as_deref(),
            Some("compositor"),
            "the waker runs on the driver"
        );
        handle.drain_events(client, &mut events);
    }
    events
}

#[test]
fn watched_animation_events_arrive_in_order_and_wake_the_client() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let mut list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let wakes = Wakes::of(&mut handle, list.client);
    handle.watch_animations(list.client, true);
    let (_, animation) = fade(&mut handle, &mut list, 30 * PERIOD_NS, 0, 2.0);

    let finished = |events: &[ClientEvent]| {
        events.iter().any(|event| {
            matches!(
                event,
                ClientEvent::Compositor(CompositorEvent::Animation {
                    event: AnimationEvent::Finished,
                    ..
                })
            )
        })
    };
    let events = wake_until(&mut handle, list.client, &wakes, finished);
    let life: Vec<(AnimationEvent, u64)> = events
        .iter()
        .filter_map(|event| match *event {
            ClientEvent::Compositor(CompositorEvent::Animation {
                id, event, at_ns, ..
            }) if id == animation => Some((event, at_ns)),
            _ => None,
        })
        .collect();
    let kinds: Vec<AnimationEvent> = life.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        [
            AnimationEvent::Started,
            AnimationEvent::Iteration,
            AnimationEvent::Finished
        ]
    );
    assert!(
        life.windows(2).all(|pair| pair[0].1 < pair[1].1),
        "{life:?}"
    );
    driver.shutdown();
}

#[test]
fn the_events_of_a_commit_are_queued_once_a_sync_returns() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let mut list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let (effect, animation) = fade(&mut handle, &mut list, 100 * PERIOD_NS, 0, f32::INFINITY);

    let mut tx = handle.transaction(list.client);
    tx.set_effect(
        effect,
        EffectNode {
            clip: None,
            opacity: 1.0,
            blend: BlendMode::NORMAL,
            filters: Arc::from([]),
            isolate: false,
        },
    );
    let events = commit(&mut handle, tx);
    assert!(events.iter().any(|event| matches!(
        *event,
        ClientEvent::Compositor(CompositorEvent::Animation {
            id,
            event: AnimationEvent::Replaced,
            ..
        }) if id == animation
    )));
    driver.shutdown();
}
