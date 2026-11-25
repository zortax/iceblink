//! The renderer of a window and the frames the driver draws into it.

use std::sync::Arc;

use iceblink::{ClientEvent, CompositorClient as _, OutputConfig, RendererFactory};
use iceblink::{FrameOutcome, OutputId};
use iceblink_wgpu::{Builder, SurfacePresenter};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::frame_log::logs_frames;
use crate::scene::Scene;

/// A compositor output whose renderer presents into a window.
pub(super) struct Output {
    id: OutputId,
    events: Vec<ClientEvent>,
    /// Whether a line is printed for each frame drawn.
    logs: bool,
}

impl Output {
    /// Returns the output `id`, which prints a line for each frame if [`logs_frames`] holds.
    pub(super) fn new(id: OutputId) -> Self {
        Self {
            id,
            events: Vec::new(),
            logs: logs_frames(),
        }
    }

    /// Configures the output and its renderer for a window of `size` at `scale`.
    pub(super) fn configure(&mut self, scene: &mut Scene, size: PhysicalSize<u32>, scale: f32) {
        scene.driver.configure_output(self.id, config(size, scale));
        println!(
            "configured size={}x{} scale={scale}",
            size.width, size.height
        );
    }

    /// Sets the refresh interval of the output's display to `refresh_ns`.
    pub(super) fn set_refresh(&mut self, scene: &mut Scene, refresh_ns: u64) {
        scene.driver.set_refresh(self.id, Some(refresh_ns));
    }

    /// Requests a frame at the output's next latch, even if nothing changed.
    pub(super) fn request_frame(&mut self, scene: &mut Scene) {
        scene.driver.request_frame(self.id);
    }

    /// Runs the frames that are due, and reports whether one was presented.
    pub(super) fn draw(&mut self, scene: &mut Scene) -> bool {
        scene.driver.pump();
        scene.events(&mut self.events);

        let mut presented = false;
        for event in self.events.drain(..) {
            match event {
                ClientEvent::Drawn { frame, outcome, .. } => {
                    presented |= matches!(outcome, FrameOutcome::Presented(_));
                    if self.logs {
                        report(frame.0, &outcome);
                    }
                }
                ClientEvent::OutputFailed { error, .. } => {
                    panic!("no device presents to the window: {error}")
                }
                _ => {}
            }
        }
        presented
    }
}

/// Returns a factory of renderers on `window`'s surface, which prints the adapter and the
/// presenter it uses.
///
/// The factory fails when no device presents to the window.
pub(super) fn renderer(window: Arc<Window>, size: PhysicalSize<u32>) -> RendererFactory {
    Box::new(move || {
        let mut renderer = Builder::new().for_surface([size.width, size.height], true, window)?;
        let adapter = renderer.gpu().describe();
        let partial = renderer.wgpu_presenter().capabilities().partial_present;
        let native = renderer
            .wgpu_presenter()
            .as_any_mut()
            .and_then(|any| any.downcast_mut::<SurfacePresenter>())
            .map(|surface| surface.is_native());
        println!("opened adapter={adapter} native={native:?} partial_present={partial}");
        Ok(Box::new(renderer))
    })
}

/// Prints the statistics of frame `frame` to standard error, or its outcome if it was not
/// presented.
fn report(frame: u64, outcome: &FrameOutcome) {
    let Some(stats) = outcome.stats() else {
        eprintln!("frame={frame} outcome={outcome:?}");
        return;
    };
    eprintln!(
        "frame={frame} presented damage_px={} copy_px={} present_damage_px={} draw_calls={} \
         uploaded={}",
        stats.damage_px,
        stats.present_copy_px,
        stats.present_damage_px,
        stats.draw_calls,
        stats.bytes_uploaded,
    );
}

/// Returns the output configuration of a window of `size` at `scale`.
pub(super) fn config(size: PhysicalSize<u32>, scale: f32) -> OutputConfig {
    OutputConfig {
        size: [size.width.max(1), size.height.max(1)],
        scale,
        opaque: true,
    }
}
