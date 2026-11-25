//! A paced driver takes the refresh interval from the first source that has one, and moves a
//! frame already due onto the refreshes whenever they become known or change.

mod common;

use std::thread;
use std::time::Duration;

use iceblink::damage::DamageSet;
use iceblink::{
    Acquired, BufferAge, Frame, FrameOutcome, MemoryReport, PresentCapabilities, PresentFeedback,
    PresentMode, PresentResult, RenderCapabilities, RenderTarget, STALE_DAMAGE, SkipReason,
};
use iceblink::{CompositorClient, Presenter, Renderer};
use iceblink_core::CompositorImpl;
use iceblink_driver::{DriverOptions, InProcess, now_ns};
use iceblink_testing::fakes::NullRenderer;

use crate::common::Scene;

const MS: u64 = 1_000_000;

/// The interval the driver assumes.
const ASSUMED_NS: u64 = 50 * MS;

/// How long before a display time a frame latches.
const BUDGET_NS: u64 = 4 * MS;

/// A presenter that reports no feedback, so the client reports when presents reached the
/// display, and that knows the display's refresh interval when it holds one.
struct Untimed {
    refresh_ns: Option<u64>,
}

static NO_DEBT: DamageSet<STALE_DAMAGE> = DamageSet::new();

impl Presenter for Untimed {
    fn capabilities(&self) -> PresentCapabilities {
        PresentCapabilities {
            mode: PresentMode::KeptTarget,
            partial_present: false,
            feedback: false,
        }
    }

    fn configure(&mut self, _size: [u32; 2], _opaque: bool) -> Result<(), SkipReason> {
        Ok(())
    }

    fn acquire(&mut self) -> Acquired {
        Acquired::Skipped(SkipReason::Unacquired)
    }

    fn acquired(&self) -> Option<u32> {
        None
    }

    fn release(&mut self, _image: u32) {}

    fn composed(&mut self, _damage: &DamageSet) {}

    fn debt(&self, _image: u32) -> &DamageSet<STALE_DAMAGE> {
        &NO_DEBT
    }

    fn present(&mut self, _image: u32, _damage: &DamageSet) -> PresentResult {
        PresentResult::Presented
    }

    fn next_deadline(&self) -> Option<u64> {
        None
    }

    fn feedback(&mut self, _out: &mut Vec<PresentFeedback>) {}

    fn refresh_ns(&self) -> Option<u64> {
        self.refresh_ns
    }
}

/// A null renderer that presents through an [`Untimed`] presenter.
struct Screen {
    inner: NullRenderer,
    presenter: Untimed,
}

impl Renderer for Screen {
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
        self.inner.draw(frame)
    }

    fn presenter(&mut self) -> Option<&mut dyn Presenter> {
        Some(&mut self.presenter)
    }

    fn memory(&self) -> MemoryReport {
        self.inner.memory()
    }
}

/// Returns a paced driver and a scene on an output whose presenter knows `refresh_ns`.
fn driver(refresh_ns: Option<u64>) -> (InProcess, Scene) {
    let options = DriverOptions {
        interval_ns: ASSUMED_NS,
        budget_ns: Some(BUDGET_NS),
        ..DriverOptions::default()
    };
    let mut driver = InProcess::new(Box::new(CompositorImpl::new()), options);
    let scene = Scene::new(
        &mut driver,
        Box::new(move || {
            Ok(Box::new(Screen {
                inner: NullRenderer::new(),
                presenter: Untimed { refresh_ns },
            }))
        }),
    );
    (driver, scene)
}

/// A display time a little over two assumed intervals ago, so the next refreshes of every
/// interval the tests use lie on different times.
fn shown_before() -> u64 {
    // The frame clock starts at its first reading.
    let ago = 130 * MS;
    thread::sleep(Duration::from_nanos(ago.saturating_sub(now_ns())));
    now_ns() - ago
}

/// Commits changes, and builds each frame that latches immediately on the idle output, until a
/// change leaves a frame due at a paced latch.
fn change_paced(driver: &mut InProcess, scene: &mut Scene) {
    for _ in 0..4 {
        scene.change(driver);
        if driver.stats().next_due_ns.is_some_and(|due| due > now_ns()) {
            return;
        }
        driver.pump();
    }
    panic!("every change latched at once");
}

/// Asserts that the frame the driver plans reaches the display on a refresh `interval_ns` apart
/// from `shown_ns`.
#[track_caller]
fn on_refreshes(driver: &InProcess, shown_ns: u64, interval_ns: u64, what: &str) {
    let display = driver.stats().next_display_ns.expect("a frame is wanted");
    assert_eq!(
        (display - shown_ns) % interval_ns,
        0,
        "{what}: the frame planned for {} ms after the display time is off the refreshes {} ms \
         apart",
        (display - shown_ns) as f64 / MS as f64,
        interval_ns / MS
    );
}

#[test]
fn the_presenters_refresh_outranks_the_clients() {
    let (mut driver, mut scene) = driver(Some(40 * MS));
    let shown = shown_before();
    driver.report_presented(scene.output, shown);
    driver.set_refresh(scene.output, Some(37 * MS));
    scene.change(&mut driver);
    on_refreshes(&driver, shown, 40 * MS, "the presenter knows the refresh");
}

#[test]
fn a_withdrawn_refresh_leaves_the_assumed_interval() {
    let (mut driver, mut scene) = driver(None);
    driver.set_refresh(scene.output, Some(37 * MS));
    let shown = shown_before();
    driver.report_presented(scene.output, shown);
    driver.set_refresh(scene.output, None);
    scene.change(&mut driver);
    on_refreshes(&driver, shown, ASSUMED_NS, "nothing measured the refresh");
}

#[test]
fn a_new_refresh_moves_the_frame_already_due() {
    let (mut driver, mut scene) = driver(None);
    driver.set_refresh(scene.output, Some(37 * MS));
    let shown = shown_before();
    driver.report_presented(scene.output, shown);
    change_paced(&mut driver, &mut scene);
    driver.set_refresh(scene.output, Some(45 * MS));
    on_refreshes(&driver, shown, 45 * MS, "the client reported a new refresh");
}

#[test]
fn the_first_display_time_moves_the_frame_already_due() {
    let (mut driver, mut scene) = driver(None);
    change_paced(&mut driver, &mut scene);
    let shown = shown_before();
    driver.report_presented(scene.output, shown);
    on_refreshes(
        &driver,
        shown,
        ASSUMED_NS,
        "the first display time is known",
    );
}
