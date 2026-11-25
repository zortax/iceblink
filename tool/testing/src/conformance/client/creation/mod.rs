//! Rules on creating outputs: the first frame, and renderers built after the call returns.

mod gate;

use std::sync::Arc;

use iceblink::{ClientEvent, GpuUnavailable, OrderNode, OutputConfig, OutputWatch};

use crate::conformance::client::Rig;
use crate::conformance::client::recorder::{SharedLog, read};
use crate::conformance::client::stage::{recorded_output, recorder, step_clock};

use self::gate::Gate;

/// An output builds no frame, however it is configured, until a commit is applied, and the first
/// commit builds one frame.
pub fn an_output_builds_no_frame_before_a_commit(rig: &Rig<'_>) {
    let mut driver = (rig.client)((rig.compositor)());
    let (client, _) = driver.register_client();
    let log = SharedLog::default();
    let output = recorded_output(&mut *driver, &log, None);
    let watch = OutputWatch {
        shown: true,
        ..OutputWatch::default()
    };
    driver.watch_output(client, output, watch);
    let config = OutputConfig {
        size: [120, 80],
        scale: 1.0,
        opaque: true,
    };
    driver.configure_output(output, config);
    for step in 0..3 {
        driver.step(step_clock(step));
    }
    driver.sync();

    assert!(
        read(&log).draws.is_empty(),
        "no frame is drawn before a commit"
    );
    let status = driver.output_status(output);
    assert!(
        status.is_some_and(|status| status.frames_built == 0),
        "no frame is built before a commit, {status:?}"
    );
    assert_eq!(
        driver.stats().next_due_ns,
        None,
        "no frame is due before a commit"
    );
    let mut events = Vec::new();
    driver.drain_events(client, &mut events);
    assert!(events.is_empty(), "nothing is presented, {events:?}");

    let mut tx = driver.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    driver.commit(tx);
    for step in 3..6 {
        driver.step(step_clock(step));
    }
    driver.sync();

    assert_eq!(
        read(&log).draws.len(),
        1,
        "the first commit builds one frame"
    );
    driver.drain_events(client, &mut events);
    let shown = events
        .iter()
        .filter(|event| matches!(event, ClientEvent::Shown { .. }))
        .count();
    assert_eq!(shown, 1, "the frame is presented, {events:?}");
}

/// A driver that draws on a thread of its own returns from `create_output` before the renderer is
/// built. A commit queued meanwhile builds its frame once the renderer is built, and a factory
/// that fails is reported with an `OutputFailed` event.
pub fn calls_queued_before_the_renderer_is_built_apply_after_it(rig: &Rig<'_>) {
    let mut driver = (rig.client)((rig.compositor)());
    let (client, _) = driver.register_client();
    let log = SharedLog::default();
    let gate = Gate::new();
    let drawn = Arc::clone(&log);
    let output = gate.create(
        &mut *driver,
        Box::new(move || Ok(Box::new(recorder(&drawn)))),
    );
    let mut tx = driver.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    driver.commit(tx);
    gate.open();
    driver.step(step_clock(0));
    driver.sync();

    assert!(
        !gate.gave_up(),
        "create_output returned before the factory ran"
    );
    assert_eq!(read(&log).draws.len(), 1, "the commit builds its frame");
    let status = driver.output_status(output);
    assert!(
        status.is_some_and(|status| status.frames_built == 1),
        "the output's snapshot is published, {status:?}"
    );

    let gate = Gate::new();
    let error = GpuUnavailable::default().rejected("the only adapter", "no surface");
    let failure = error.clone();
    let failed = gate.create(&mut *driver, Box::new(move || Err(failure)));
    let tx = driver.transaction(client);
    driver.commit(tx);
    gate.open();
    driver.sync();

    assert!(
        !gate.gave_up(),
        "create_output returned before the factory ran"
    );
    let mut events = Vec::new();
    driver.drain_events(client, &mut events);
    assert!(
        events.contains(&ClientEvent::OutputFailed {
            output: failed,
            error
        }),
        "the failure names the output, {events:?}"
    );
}
