//! The window, its event loop and the frames drawn into it.

mod output;
mod shown;
mod wheel;

use std::sync::Arc;
use std::time::{Duration, Instant};

use iceblink_driver::now_ns;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::demo::Demo;

use self::shown::Shown;

/// Opens a window showing `D` and runs until the window is closed.
///
/// # Panics
///
/// Panics if no event loop, window or graphics device can be made.
pub fn run<D: Demo>() {
    crate::log::install();
    let event_loop = EventLoop::new().expect("an event loop opens");
    let mut app = App::<D> { shown: None };
    event_loop.run_app(&mut app).expect("the event loop runs");
}

struct App<D> {
    shown: Option<Shown<D>>,
}

impl<D: Demo> ApplicationHandler for App<D> {
    // The work that is due runs once the window's events are handled, and the loop sleeps until
    // the next is due.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let wake = self.shown.as_mut().and_then(Shown::run_due);
        let flow = wake.map_or(ControlFlow::Wait, |wake_ns| {
            let wait = Duration::from_nanos(wake_ns.saturating_sub(now_ns()));
            ControlFlow::WaitUntil(Instant::now() + wait)
        });
        event_loop.set_control_flow(flow);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shown.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(D::TITLE)
            .with_inner_size(LogicalSize::new(800.0, 600.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("a window opens"),
        );
        self.shown = Some(Shown::open(window));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(shown) = self.shown.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => shown.resize(size),
            WindowEvent::ScaleFactorChanged { .. } => shown.resize(shown.window.inner_size()),
            WindowEvent::MouseWheel { delta, .. } => shown.wheel(delta),
            _ => {}
        }
    }
}
