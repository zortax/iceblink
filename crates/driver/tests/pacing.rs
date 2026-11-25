//! An idle output sleeps until a wake time or a call, and a held frame is presented once the
//! output is idle.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use iceblink::{AnimationEvent, ClientId, CompositorEvent, FrameOutcome, FrameStats, SkipReason};
use iceblink::{ClientEvent, CompositorClient, DriverPacing};
use iceblink_driver::thread::ClientHandle;

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, commit, fade, spawn};
use crate::common::thread::wake::Wakes;
use crate::common::thread::watched::Watched;

#[test]
fn a_paced_thread_paces_itself() {
    let (driver, handle) = spawn();
    assert_eq!(
        handle.pacing(),
        DriverPacing::SelfPaced,
        "the client neither steps nor pumps"
    );
    driver.shutdown();
}

#[test]
fn a_delayed_animation_starts_without_another_call() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let mut list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let wakes = Wakes::of(&mut handle, list.client);
    handle.watch_animations(list.client, true);
    let delay = 20 * PERIOD_NS;
    let (_, animation) = fade(&mut handle, &mut list, 10 * PERIOD_NS, delay as i64, 1.0);

    // The client makes no further call: only the wake time can start the animation.
    let mut events = Vec::new();
    let started = loop {
        let started = events.iter().find_map(|event| match *event {
            ClientEvent::Compositor(CompositorEvent::Animation {
                id,
                event: AnimationEvent::Started,
                at_ns,
                ..
            }) if id == animation => Some(at_ns),
            _ => None,
        });
        if let Some(at_ns) = started {
            break at_ns;
        }
        wakes.next();
        handle.drain_events(list.client, &mut events);
    };
    let draws = probe.wait_draws(1);
    assert!(draws.iter().any(|drawn| drawn.clock.present_ns >= started));
    driver.shutdown();
}

#[test]
fn a_held_frame_is_presented_once_the_output_is_idle() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    probe.wait_draws(1);

    probe.update(|state| {
        state
            .script
            .push_back(FrameOutcome::Skipped(SkipReason::Held));
    });
    let mut tx = handle.transaction(list.client);
    tx.scroll_to(list.node, [0.0, 50.0]);
    commit(&mut handle, tx);

    let held = probe.wait(|state| state.held.first().copied());
    assert_eq!(held, FrameOutcome::Presented(FrameStats::default()));
    driver.shutdown();
}

#[test]
fn a_blocking_call_from_the_waker_panics_on_the_driver_thread() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let mut list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let caller = handle.clone();
    let output = list.output;
    let waker = std::sync::Arc::new(move || {
        caller.clone().hit_test(output, [0.0, 0.0]);
    });
    handle.set_waker(list.client, Some(waker));
    handle.watch_animations(list.client, true);
    fade(&mut handle, &mut list, 10 * PERIOD_NS, 0, 1.0);
    // The animation's first event calls the waker, which stops the driver.
    while !driver_stopped(&mut handle, list.client) {
        std::thread::sleep(std::time::Duration::from_nanos(PERIOD_NS));
    }

    let stopped = catch_unwind(AssertUnwindSafe(|| driver.shutdown()));
    let panic = stopped.expect_err("the waker's call panicked on the driver thread");
    let message = panic.downcast_ref::<&str>().copied().unwrap_or_default();
    assert!(message.contains("never returns"), "{message}");
}

/// Reports whether a commit of `client` is rejected because the driver stopped.
fn driver_stopped(handle: &mut ClientHandle, client: ClientId) -> bool {
    let tx = handle.transaction(client);
    handle.commit(tx);
    let mut events = Vec::new();
    handle.drain_events(client, &mut events);
    events
        .iter()
        .any(|event| matches!(event, ClientEvent::Rejected { .. }))
}
