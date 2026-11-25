//! A window with the scene it shows, and the frames and animation steps that are due.

use std::sync::Arc;

use iceblink::CompositorClient as _;
use iceblink_driver::{DEFAULT_INTERVAL_NS, now_ns};
use winit::dpi::PhysicalSize;
use winit::event::MouseScrollDelta;
use winit::window::Window;

use crate::demo::Demo;
use crate::monitor::refresh_ns;
use crate::scene::{Scene, scaled};

use super::output::{self, Output};
use super::wheel::Wheel;

/// How far one line of a wheel scrolls, in logical pixels.
const LINE: f32 = 40.0;

/// A window with the scene it shows and the renderer that draws it.
pub(super) struct Shown<D> {
    pub(super) window: Arc<Window>,
    scene: Scene,
    output: Output,
    demo: D,
    wheel: Wheel,
    /// When the window opened, on the frame clock.
    opened_ns: u64,
    /// The refresh interval of the monitor the window is shown on, if it reports one.
    refresh_ns: Option<u64>,
    /// Whether the demo or the wheel's ease changes the content again.
    animating: bool,
    /// When the demo is advanced next while no frame is due, on the frame clock.
    advance_ns: Option<u64>,
}

impl<D: Demo> Shown<D> {
    /// Opens the scene and the demo on `window`, and follows the monitor's refresh.
    pub(super) fn open(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        let renderer = output::renderer(Arc::clone(&window), size);
        let (mut scene, output) = Scene::open(output::config(size, scale), D::fonts(), renderer);
        let output = Output::new(output);

        let mut tx = scene.transaction();
        let demo = D::new(&mut scene, &mut tx, logical(size, scale));
        scene.commit(tx);

        let now = now_ns();
        let mut shown = Self {
            window,
            scene,
            output,
            demo,
            wheel: Wheel::default(),
            opened_ns: now,
            refresh_ns: None,
            animating: true,
            advance_ns: Some(now),
        };
        shown.follow_monitor();
        shown
    }

    /// Configures the output for a window of `size` and lays the demo out again.
    ///
    /// A window with no area is left as it is.
    pub(super) fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.follow_monitor();
        let scale = self.window.scale_factor() as f32;
        self.output.configure(&mut self.scene, size, scale);

        let mut tx = self.scene.transaction();
        tx.set_transform(self.scene.space(), scaled(scale));
        self.demo.resize(&self.scene, &mut tx, logical(size, scale));
        self.scene.commit(tx);
    }

    /// Scrolls the demo by a wheel's `delta`.
    ///
    /// The lines of a wheel are spread over a short ease. Pixel deltas apply immediately.
    pub(super) fn wheel(&mut self, delta: MouseScrollDelta) {
        // Winit's deltas are positive when the content moves right and down.
        match delta {
            MouseScrollDelta::LineDelta(x, y) => {
                let now = now_ns();
                self.wheel.push([-x * LINE, -y * LINE], now);
                self.animating = true;
                self.advance_ns = Some(now);
            }
            MouseScrollDelta::PixelDelta(at) => {
                let scale = self.window.scale_factor();
                let delta = [(-at.x / scale) as f32, (-at.y / scale) as f32];
                let mut tx = self.scene.transaction();
                self.demo.wheel(&mut tx, delta);
                self.scene.commit(tx);
            }
        }
    }

    /// Advances the demo and runs the frames that are due, and returns when work is due next,
    /// on the frame clock, or `None` when none is wanted.
    ///
    /// While the content changes, every frame presented wants the next one, so the demo is
    /// advanced at every refresh.
    pub(super) fn run_due(&mut self) -> Option<u64> {
        let now = now_ns();
        let frame_due = self
            .scene
            .driver
            .stats()
            .next_due_ns
            .is_some_and(|due| due <= now);

        if frame_due || self.advance_ns.is_some_and(|at| at <= now) {
            self.advance_ns = None;
            if self.animating {
                self.advance(now);
            }
            let presented = self.output.draw(&mut self.scene);
            if self.animating {
                if presented {
                    self.output.request_frame(&mut self.scene);
                } else {
                    // A frame that was not presented wants no next frame, so the demo is
                    // advanced again one refresh later.
                    self.advance_ns = Some(now + self.interval_ns());
                }
            }
        }

        let due = self.scene.driver.stats().next_due_ns;
        match (due, self.advance_ns) {
            (Some(due), Some(advance)) => Some(due.min(advance)),
            (due, advance) => due.or(advance),
        }
    }

    /// Advances the demo and the wheel's ease to the display time of the frame that a commit at
    /// `now_ns` goes into.
    fn advance(&mut self, now_ns: u64) {
        let shown_ns = self
            .scene
            .driver
            .stats()
            .commit_display_ns
            .unwrap_or(now_ns + self.interval_ns());

        let seconds = shown_ns.saturating_sub(self.opened_ns) as f32 / 1e9;
        let mut tx = self.scene.transaction();
        let animating = self.demo.advance(&mut tx, seconds);
        if let Some(step) = self.wheel.take(shown_ns) {
            self.demo.wheel(&mut tx, step);
        }
        self.animating = animating || self.wheel.moving();
        if !tx.is_empty() {
            self.scene.commit(tx);
        }
    }

    /// Takes the refresh interval of the monitor the window is shown on as the output's, when it
    /// changed.
    fn follow_monitor(&mut self) {
        let refresh = refresh_ns(&self.window);
        if let Some(refresh) = refresh
            && refresh != self.refresh_ns.unwrap_or(0)
        {
            self.output.set_refresh(&mut self.scene, refresh);
            self.refresh_ns = Some(refresh);
        }
    }

    /// Returns the refresh interval of the monitor, or the driver's default interval if it reports
    /// none.
    fn interval_ns(&self) -> u64 {
        self.refresh_ns.unwrap_or(DEFAULT_INTERVAL_NS)
    }
}

/// Returns `size` in logical pixels at `scale`.
fn logical(size: PhysicalSize<u32>, scale: f32) -> [f32; 2] {
    [size.width as f32 / scale, size.height as f32 / scale]
}
