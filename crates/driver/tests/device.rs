//! A lost device parks its output until a frame is asked for, and a recovered one is redrawn
//! whole.

mod common;

use std::thread;
use std::time::Duration;

use iceblink::CompositorClient;
use iceblink::{FrameOutcome, SkipReason};

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, fade, spawn};
use crate::common::thread::watched::Watched;

#[test]
fn an_unavailable_device_parks_the_output_until_a_frame_is_requested() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let mut list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    fade(&mut handle, &mut list, 100 * PERIOD_NS, 0, f32::INFINITY);
    let running = probe.draws() + 2;
    probe.wait_draws(running);

    probe.update(|state| {
        state
            .script
            .push_back(FrameOutcome::Skipped(SkipReason::DeviceUnavailable));
    });
    let lost = probe.wait(|state| {
        state.draws.iter().rposition(|drawn| {
            drawn.outcome == FrameOutcome::Skipped(SkipReason::DeviceUnavailable)
        })
    });
    // The animation still runs, but nothing draws on a device that is gone.
    thread::sleep(Duration::from_nanos(10 * PERIOD_NS));
    assert_eq!(probe.draws(), lost + 1);

    handle.request_frame(list.output);
    let draws = probe.wait_draws(lost + 3);
    assert!(
        draws[lost + 1].full,
        "the first frame after the loss is whole"
    );
    assert!(!draws[lost + 2].full);
    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}

#[test]
fn a_recovered_device_is_redrawn_whole_at_the_next_frame() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let mut list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    fade(&mut handle, &mut list, 100 * PERIOD_NS, 0, f32::INFINITY);
    probe.wait_draws(probe.draws() + 2);

    probe.update(|state| state.script.push_back(FrameOutcome::Recovered));
    let recovered = probe.wait(|state| {
        state
            .draws
            .iter()
            .rposition(|drawn| drawn.outcome == FrameOutcome::Recovered)
    });
    let draws = probe.wait_draws(recovered + 2);
    assert!(draws[recovered + 1].full);
    assert!(probe.state().violations.is_empty());
    driver.shutdown();
}
