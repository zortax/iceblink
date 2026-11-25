//! Shutting the driver down: what it applies first, where renderers are dropped, and what calls
//! answer afterwards.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use iceblink::{ClientEvent, CompositorClient, OutputConfig};
use iceblink::{GesturePhase, RejectReason, ScrollRouting};

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, spawn};
use crate::common::thread::watched::Watched;

#[test]
fn shutdown_applies_the_queued_calls_and_drops_renderers_on_the_driver_thread() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let mut events = Vec::new();
    handle.drain_events(list.client, &mut events);
    let called: Vec<_> = (0..20)
        .map(|_| {
            let tx = handle.transaction(list.client);
            handle.commit(tx)
        })
        .collect();
    driver.shutdown();

    events.clear();
    handle.drain_events(list.client, &mut events);
    let applied: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ClientEvent::Committed { commit, .. } => Some(*commit),
            _ => None,
        })
        .collect();
    assert_eq!(
        applied, called,
        "the commits queued before the shutdown are applied"
    );
    let state = probe.state();
    let driver_thread = state.built_on.expect("the renderer was built");
    assert_ne!(driver_thread, thread::current().id());
    assert_eq!(state.dropped_on, Some(driver_thread));
}

#[test]
fn calls_after_shutdown_answer_as_a_stopped_compositor() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    driver.shutdown();
    let mut events = Vec::new();
    handle.drain_events(list.client, &mut events);
    events.clear();

    let tx = handle.transaction(list.client);
    let stopped = handle.commit(tx);
    list.gesture(&mut handle, 10.0, GesturePhase::Begin);
    handle.drain_events(list.client, &mut events);
    assert!(
        matches!(
            &events[..],
            [
                ClientEvent::Rejected { commit, error },
                ClientEvent::Rerouted { routing: ScrollRouting::Unhandled, .. },
            ] if *commit == stopped && error.reason == RejectReason::Stopped
        ),
        "the commit is rejected and the gesture unhandled, events {events:?}"
    );
    assert!(handle.hit_test(list.output, [100.0, 100.0]).is_empty());
    assert!(handle.capabilities().threaded);
    handle.sync();
}

#[test]
fn a_panic_on_the_driver_thread_reaches_shutdown() {
    let (driver, mut handle) = spawn();
    let config = OutputConfig {
        size: [10, 10],
        scale: 1.0,
        opaque: true,
    };
    handle.create_output(config, Box::new(|| panic!("no renderer today")));
    handle.sync();

    let stopped = catch_unwind(AssertUnwindSafe(|| driver.shutdown()));
    let panic = stopped.expect_err("the driver's panic is raised again");
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"no renderer today"));
}

#[test]
fn commits_queued_behind_a_panic_are_rejected_as_stopped() {
    let (driver, mut handle) = spawn();
    let (client, _) = handle.register_client();
    // Whether the factory runs, and whether it may panic.
    let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
    let held = Arc::clone(&gate);
    let mut creating = handle.clone();
    let creator = thread::spawn(move || {
        let config = OutputConfig {
            size: [10, 10],
            scale: 1.0,
            opaque: true,
        };
        let factory = Box::new(move || -> Result<Box<dyn iceblink::Renderer>, _> {
            let (state, changed) = &*held;
            let mut state = state.lock().unwrap();
            state.0 = true;
            changed.notify_all();
            let _open = changed.wait_while(state, |state| !state.1).unwrap();
            panic!("no renderer today")
        });
        creating.create_output(config, factory);
    });
    let (state, changed) = &*gate;
    drop(
        changed
            .wait_while(state.lock().unwrap(), |state| !state.0)
            .unwrap(),
    );

    // The driver is inside the factory, so these commits wait in the queue behind it.
    let queued: Vec<_> = (0..5)
        .map(|_| {
            let tx = handle.transaction(client);
            handle.commit(tx)
        })
        .collect();
    state.lock().unwrap().1 = true;
    changed.notify_all();
    creator.join().unwrap();
    let stopped = catch_unwind(AssertUnwindSafe(|| driver.shutdown()));
    assert!(stopped.is_err(), "the driver's panic is raised again");

    let mut events = Vec::new();
    handle.drain_events(client, &mut events);
    let rejected: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ClientEvent::Rejected { commit, error } if error.reason == RejectReason::Stopped => {
                Some(*commit)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        rejected, queued,
        "every queued commit is answered as stopped"
    );
}
