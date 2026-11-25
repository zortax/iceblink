//! A stepped driver builds the frame a client's call wants at the next step, whatever its clock.

mod common;

use iceblink::CompositorClient;
use iceblink::FrameClock;
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess};
use iceblink_testing::fakes::NullRenderer;

use crate::common::Scene;

const INTERVAL_NS: u64 = 16_000_000;

fn at(now_ns: u64) -> FrameClock {
    FrameClock {
        now_ns,
        present_ns: now_ns + INTERVAL_NS,
        interval_ns: INTERVAL_NS,
    }
}

#[test]
fn a_commit_is_built_at_a_step_with_an_earlier_clock() {
    let mut driver = InProcess::new(Box::new(CompositorImpl::new()), DriverOptions::stepped());
    let mut scene = Scene::new(&mut driver, Box::new(|| Ok(Box::new(NullRenderer::new()))));
    scene.change(&mut driver);
    driver.step(at(10 * INTERVAL_NS));
    assert_eq!(driver.stats().frames_built, 1, "the first commit is built");

    scene.change(&mut driver);
    driver.step(at(4 * INTERVAL_NS));
    assert_eq!(
        driver.stats().frames_built,
        2,
        "a commit is built at the next step, though its clock is earlier than the last frame's"
    );
}
