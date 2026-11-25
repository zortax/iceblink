//! A renderer that records every frame it draws into a log a case reads.

use std::any::Any;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use iceblink::{
    BufferAge, Frame, FrameClock, FrameOutcome, MemoryReport, RenderCapabilities, RenderTarget,
};
use iceblink::{Presenter, Renderer};

use crate::conformance::client::flaky::Flaky;
use crate::fakes::{Draw, NullRenderer};
use crate::validate::FrameViolation;

/// The frames a stage's renderer drew and the frame contract violations found in them.
#[derive(Debug, Default)]
pub struct Log {
    /// Every frame drawn, oldest first.
    pub draws: Vec<Recorded>,
    /// Every violation found in those frames.
    pub violations: Vec<FrameViolation>,
    /// The releases the renderer was asked for, oldest first.
    pub released: Vec<Release>,
}

/// A release of renderer resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// `Renderer::release_cached_targets`.
    CachedTargets,
    /// `Renderer::release_idle_resources`.
    IdleResources,
}

/// One frame a stage's renderer drew.
#[derive(Clone, Debug, PartialEq)]
pub struct Recorded {
    /// The frame's id, output, damage and shifts, and the outcome it was answered with.
    pub draw: Draw,
    /// The clock the frame was built with.
    pub clock: FrameClock,
    /// The output's size the frame was built for, in device pixels.
    pub size: [u32; 2],
    /// How many quads the frame draws.
    pub quads: usize,
}

/// A shared [`Log`].
pub type SharedLog = Arc<Mutex<Log>>;

/// Locks `log`.
pub(crate) fn read(log: &SharedLog) -> MutexGuard<'_, Log> {
    log.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A validating [`NullRenderer`] that copies each frame it draws into a shared log.
pub(crate) struct Recorder {
    inner: NullRenderer,
    log: SharedLog,
    presenter: Option<Flaky>,
}

impl Recorder {
    pub(crate) fn new(caps: RenderCapabilities, log: SharedLog) -> Self {
        Self {
            inner: NullRenderer::with_capabilities(caps).validating(),
            log,
            presenter: None,
        }
    }

    /// Returns the recorder drawing through `presenter`, in its buffer-age mode.
    pub(crate) fn presenting(mut self, presenter: Flaky) -> Self {
        self.presenter = Some(presenter);
        self
    }

    /// Answers the next frame with `outcome`.
    pub(crate) fn push_outcome(&mut self, outcome: FrameOutcome) {
        self.inner.push_outcome(outcome);
    }
}

impl Renderer for Recorder {
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
        let outcome = self.inner.draw(frame);

        let mut log = read(&self.log);
        if let Some(draw) = self.inner.draws().last() {
            log.draws.push(Recorded {
                draw: draw.clone(),
                clock: frame.clock,
                size: frame.size,
                quads: frame.lanes.quads.drawn(),
            });
        }
        if let Some(fresh) = self.inner.violations().get(log.violations.len()..) {
            log.violations.extend_from_slice(fresh);
        }
        outcome
    }

    fn presenter(&mut self) -> Option<&mut dyn Presenter> {
        self.presenter
            .as_mut()
            .map(|presenter| presenter as &mut dyn Presenter)
    }

    fn present_held(&mut self) -> FrameOutcome {
        self.inner.present_held()
    }

    fn memory(&self) -> MemoryReport {
        self.inner.memory()
    }

    fn release_cached_targets(&mut self) -> u64 {
        read(&self.log).released.push(Release::CachedTargets);
        0
    }

    fn release_idle_resources(&mut self) -> u64 {
        read(&self.log).released.push(Release::IdleResources);
        0
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}
