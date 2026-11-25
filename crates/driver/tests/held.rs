//! A frame composed and held back stays held until the platform takes it.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use iceblink::{
    BufferAge, Frame, FrameClock, FrameOutcome, FrameStats, MemoryReport, RenderCapabilities,
    RenderTarget, SkipReason,
};
use iceblink::{CompositorClient, Renderer};
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess};
use iceblink_testing::fakes::NullRenderer;

use crate::common::Scene;

const INTERVAL_NS: u64 = 16_000_000;

/// A null renderer that holds every frame it draws, and answers `present_held` from a script.
struct Holding {
    inner: NullRenderer,
    /// The outcomes of the next `present_held` calls, first to last.
    script: Arc<Mutex<Vec<FrameOutcome>>>,
    presented: Arc<AtomicUsize>,
}

impl Renderer for Holding {
    fn capabilities(&self) -> RenderCapabilities {
        self.inner.capabilities()
    }

    fn configure(&mut self, target: RenderTarget) {
        self.inner.configure(target);
    }

    fn target(&self) -> Option<RenderTarget> {
        self.inner.target()
    }

    fn target_age(&self) -> BufferAge {
        self.inner.target_age()
    }

    fn draw(&mut self, frame: &Frame) -> FrameOutcome {
        self.inner.draw(frame);
        FrameOutcome::Skipped(SkipReason::Held)
    }

    fn present_held(&mut self) -> FrameOutcome {
        let mut script = self.script.lock().unwrap();
        let outcome = if script.is_empty() {
            FrameOutcome::Presented(FrameStats::default())
        } else {
            script.remove(0)
        };
        if matches!(outcome, FrameOutcome::Presented(_)) {
            self.presented.fetch_add(1, Ordering::Relaxed);
        }
        outcome
    }

    fn memory(&self) -> MemoryReport {
        self.inner.memory()
    }
}

fn clock(step: u64) -> FrameClock {
    FrameClock {
        now_ns: step * INTERVAL_NS,
        present_ns: (step + 1) * INTERVAL_NS,
        interval_ns: INTERVAL_NS,
    }
}

#[test]
fn a_held_frame_the_platform_cannot_take_yet_is_presented_by_a_later_frame() {
    let script = Arc::new(Mutex::new(vec![
        FrameOutcome::Skipped(SkipReason::Held),
        FrameOutcome::Skipped(SkipReason::Unacquired),
    ]));
    let presented = Arc::new(AtomicUsize::new(0));
    let mut driver = InProcess::new(Box::new(CompositorImpl::new()), DriverOptions::stepped());
    let renderer = {
        let script = Arc::clone(&script);
        let presented = Arc::clone(&presented);
        Box::new(move || {
            Ok(Box::new(Holding {
                inner: NullRenderer::new(),
                script,
                presented,
            }) as Box<dyn Renderer>)
        })
    };
    let mut scene = Scene::new(&mut driver, renderer);
    scene.change(&mut driver);

    for step in 0..6 {
        driver.step(clock(step));
    }
    assert_eq!(
        presented.load(Ordering::Relaxed),
        1,
        "the held frame is presented once the platform takes it, after it could not acquire an \
         image"
    );
    assert_eq!(
        driver.stats().next_due_ns,
        None,
        "once presented, the output wants no frame"
    );
}
