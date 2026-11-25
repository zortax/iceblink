//! A renderer that prints a line for every frame it draws.

use std::any::Any;

use iceblink::{BufferAge, Frame, FrameOutcome, MemoryReport, RenderCapabilities, RenderTarget};
use iceblink::{Presenter, Renderer};

/// A renderer that draws through `inner` and prints each frame's outcome to standard error.
pub(crate) struct Logged<R> {
    pub(crate) inner: R,
}

impl<R: Renderer> Renderer for Logged<R> {
    fn capabilities(&self) -> RenderCapabilities {
        self.inner.capabilities()
    }

    fn configure(&mut self, target: RenderTarget) {
        self.inner.configure(target);
        println!(
            "configured size={}x{} scale={}",
            target.size[0], target.size[1], target.scale
        );
    }

    fn target(&self) -> Option<RenderTarget> {
        self.inner.target()
    }

    fn target_age(&self) -> BufferAge {
        self.inner.target_age()
    }

    fn draw(&mut self, frame: &Frame) -> FrameOutcome {
        let outcome = self.inner.draw(frame);
        let present_ms = frame.clock.present_ns as f64 / 1e6;
        match outcome.stats() {
            Some(stats) => eprintln!(
                "frame={} present_ms={present_ms:.1} damage_px={} shifts={} draw_calls={}",
                frame.id.0,
                stats.damage_px,
                frame.shifts.len(),
                stats.draw_calls,
            ),
            None => eprintln!(
                "frame={} present_ms={present_ms:.1} outcome={outcome:?}",
                frame.id.0
            ),
        }
        outcome
    }

    fn present_held(&mut self) -> FrameOutcome {
        self.inner.present_held()
    }

    fn presenter(&mut self) -> Option<&mut dyn Presenter> {
        self.inner.presenter()
    }

    fn memory(&self) -> MemoryReport {
        self.inner.memory()
    }

    fn as_any(&self) -> Option<&dyn Any> {
        self.inner.as_any()
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        self.inner.as_any_mut()
    }
}
