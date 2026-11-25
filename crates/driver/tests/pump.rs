//! A paced in-process driver builds a frame in `pump` once the frame latched, and not before.

use std::thread;
use std::time::Duration;

use iceblink::{ClientEvent, CompositorClient, DriverPacing, OutputConfig};
use iceblink::{ClientId, OrderNode};
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess, now_ns};
use iceblink_testing::fakes::NullRenderer;

/// A refresh interval long enough that the test thread's timing never decides the result.
const INTERVAL_NS: u64 = 200_000_000;

fn drawn(driver: &mut InProcess, client: ClientId) -> usize {
    let mut events = Vec::new();
    driver.drain_events(client, &mut events);
    events
        .iter()
        .filter(|event| matches!(event, ClientEvent::Drawn { .. }))
        .count()
}

#[test]
fn pump_builds_a_frame_once_it_latched() {
    let options = DriverOptions {
        interval_ns: INTERVAL_NS,
        budget_ns: Some(INTERVAL_NS / 10),
        ..DriverOptions::default()
    };
    let mut driver = InProcess::new(Box::new(CompositorImpl::new()), options);
    assert_eq!(
        driver.pacing(),
        DriverPacing::Pumped,
        "the client must pump"
    );
    let (client, _) = driver.register_client();
    let config = OutputConfig {
        size: [64, 64],
        scale: 1.0,
        opaque: true,
    };
    let output = driver.create_output(config, Box::new(|| Ok(Box::new(NullRenderer::new()))));
    driver.watch_frames(client, output, true);
    let mut tx = driver.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    driver.commit(tx);
    driver.pump();
    assert_eq!(drawn(&mut driver, client), 1, "a new output draws at once");

    let mut tx = driver.transaction(client);
    tx.set_root(output, None);
    driver.commit(tx);
    let due = driver
        .stats()
        .next_due_ns
        .expect("the commit wants a frame");
    assert!(
        due > now_ns(),
        "the frame latches one interval after the last one"
    );
    driver.pump();
    assert_eq!(
        drawn(&mut driver, client),
        0,
        "pump builds nothing before the latch"
    );

    thread::sleep(Duration::from_nanos(
        due.saturating_sub(now_ns()) + 1_000_000,
    ));
    driver.pump();
    assert_eq!(
        drawn(&mut driver, client),
        1,
        "pump builds the frame once it latched"
    );
}
