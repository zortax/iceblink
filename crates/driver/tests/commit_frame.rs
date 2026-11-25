//! A driver reports the frame a client's commit goes into only when that frame is due, and never
//! a later frame the compositor wants for itself.

use std::thread;
use std::time::Duration;

use iceblink::{Anchor, Matrix4, OrderNode, TransformNode};
use iceblink::{CompositorClient, OutputConfig, now_ns};
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess};
use iceblink_testing::fakes::NullRenderer;

/// A refresh interval long enough that the test thread's timing never decides the result, and
/// short enough that a node coming to rest is more than one interval away.
const INTERVAL_NS: u64 = 25_000_000;

const BUDGET_NS: u64 = INTERVAL_NS / 10;

/// Pumps `driver` once its next frame is due.
fn pump_due(driver: &mut InProcess) {
    let due = driver.stats().next_due_ns.expect("a frame is due");
    thread::sleep(Duration::from_nanos(due.saturating_sub(now_ns())));
    driver.pump();
}

#[test]
fn the_frame_a_commit_goes_into_is_reported_and_the_compositors_own_frame_is_not() {
    let options = DriverOptions {
        interval_ns: INTERVAL_NS,
        budget_ns: Some(BUDGET_NS),
        ..DriverOptions::default()
    };
    let mut driver = InProcess::new(Box::new(CompositorImpl::new()), options);
    let (client, _) = driver.register_client();
    let config = OutputConfig {
        size: [64, 64],
        scale: 1.0,
        opaque: true,
    };
    let output = driver.create_output(config, Box::new(|| Ok(Box::new(NullRenderer::new()))));
    let mut tx = driver.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    let node = TransformNode {
        parent: None,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: Anchor::Scrolls,
    };
    let space = tx.create_transform(node);
    driver.commit(tx);
    pump_due(&mut driver);
    let idle = driver.stats();
    assert_eq!(
        (idle.commit_due_ns, idle.commit_display_ns),
        (None, None),
        "with no frame due, a commit latches at once or at the next refresh"
    );

    // A slight scale moves the node, which comes to rest 100 ms after the frame before.
    let mut tx = driver.transaction(client);
    tx.set_transform(space, Matrix4::scale(0.95, 0.95, 1.0));
    driver.commit(tx);
    pump_due(&mut driver);

    let stats = driver.stats();
    let now = now_ns();
    let rest = stats.next_display_ns.expect("the node comes to rest");
    assert!(
        rest > now + 2 * INTERVAL_NS,
        "the driver's next frame is the rest {} ms ahead",
        (rest - now) / 1_000_000
    );
    assert_eq!(
        (stats.commit_due_ns, stats.commit_display_ns),
        (None, None),
        "a commit does not go into the frame that brings the node to rest"
    );

    let mut tx = driver.transaction(client);
    tx.set_transform(space, Matrix4::scale(0.96, 0.96, 1.0));
    driver.commit(tx);
    let stats = driver.stats();
    let now = now_ns();
    assert_eq!(stats.commit_due_ns, stats.next_due_ns);
    assert_eq!(stats.commit_display_ns, stats.next_display_ns);
    let reach = stats.commit_display_ns.expect("the commit wants a frame");
    assert!(
        reach <= now + INTERVAL_NS + BUDGET_NS,
        "the commit's frame is shown {} ms ahead, and the next refresh at most {} ms",
        reach.saturating_sub(now) / 1_000_000,
        (INTERVAL_NS + BUDGET_NS) / 1_000_000
    );
}
