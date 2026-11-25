//! A paced driver's display time is on the frame clock that `iceblink::now_ns` reads, and
//! the frame it predicts is built for that time.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use iceblink::OrderNode;
use iceblink::{CompositorClient, OutputConfig, now_ns};
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess};
use iceblink_testing::fakes::NullRenderer;

/// A refresh interval long enough that the test thread's timing never decides the result.
const INTERVAL_NS: u64 = 200_000_000;

#[test]
fn the_next_display_time_is_the_frame_clock_time_the_frame_is_built_for() {
    let options = DriverOptions {
        interval_ns: INTERVAL_NS,
        budget_ns: Some(INTERVAL_NS / 10),
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
    let built = Arc::new(Mutex::new(Vec::new()));
    let noted = Arc::clone(&built);
    driver.observe_frames(
        output,
        Some(Box::new(move |frame| {
            noted.lock().unwrap().push(frame.clock.present_ns);
        })),
    );
    let mut tx = driver.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(output, Some(root));
    driver.commit(tx);
    driver.pump();

    let mut tx = driver.transaction(client);
    tx.set_root(output, None);
    driver.commit(tx);
    let stats = driver.stats();
    let now = now_ns();
    let display = stats.next_display_ns.expect("the commit wants a frame");
    let due = stats.next_due_ns.expect("the commit wants a frame");
    assert!(
        display > now && display <= now + INTERVAL_NS + INTERVAL_NS / 10,
        "the display time {display} is on the frame clock, which reads {now}"
    );
    thread::sleep(Duration::from_nanos(due.saturating_sub(now_ns())));
    driver.pump();

    assert_eq!(
        built.lock().unwrap().last().copied(),
        Some(display),
        "the frame is built for the display time the driver published"
    );
}
