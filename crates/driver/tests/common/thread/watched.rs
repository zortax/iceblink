//! A renderer that draws nothing, paced by a fake display, and reports every frame to a probe.

use std::any::Any;
use std::sync::Arc;
use std::thread;

use iceblink::{BufferAge, Frame, FrameOutcome, MemoryReport, RenderCapabilities, RenderTarget};
use iceblink::{Presenter, Renderer, RendererFactory};
use iceblink_cpu::PixmapPresenter;
use iceblink_testing::fakes::NullRenderer;

use crate::common::thread::probe::{Drawn, Probe};
use crate::common::thread::ticks::Ticks;

/// A validating `NullRenderer` that presents through [`Ticks`], answers frames with the probe's
/// script while it holds outcomes, and reports each frame and its own drop to the probe.
pub(crate) struct Watched {
    inner: NullRenderer,
    presenter: Ticks,
    probe: Arc<Probe>,
}

impl Watched {
    /// Returns a factory of renderers paced at `period_ns` that report to `probe`.
    pub(crate) fn factory(probe: &Arc<Probe>, period_ns: u64) -> RendererFactory {
        Self::paced_by(probe, period_ns, true, None)
    }

    /// Returns a factory of renderers whose presenters report the refreshes of a display with
    /// period `period_ns` only as feedback.
    pub(crate) fn without_deadlines(probe: &Arc<Probe>, period_ns: u64) -> RendererFactory {
        Self::paced_by(probe, period_ns, false, None)
    }

    /// Returns a factory of renderers whose presenters have no deadlines and report each present
    /// at the first refresh of period `period_ns` at least `takes_ns` after it.
    pub(crate) fn showing_presents(
        probe: &Arc<Probe>,
        period_ns: u64,
        takes_ns: u64,
    ) -> RendererFactory {
        Self::paced_by(probe, period_ns, false, Some(takes_ns))
    }

    fn paced_by(
        probe: &Arc<Probe>,
        period_ns: u64,
        deadlines: bool,
        takes_ns: Option<u64>,
    ) -> RendererFactory {
        let probe = Arc::clone(probe);
        Box::new(move || {
            probe.built_here();
            let mut presenter =
                Ticks::new(PixmapPresenter::kept([1, 1]), period_ns, Arc::clone(&probe));
            if !deadlines {
                presenter = presenter.without_deadlines();
            }
            if let Some(takes) = takes_ns {
                presenter = presenter.showing_presents(takes);
            }
            Ok(Box::new(Self {
                inner: NullRenderer::new().validating(),
                presenter,
                probe,
            }))
        })
    }
}

impl Renderer for Watched {
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
        if let Some(outcome) = self.probe.state().script.pop_front() {
            self.inner.push_outcome(outcome);
        }
        let outcome = self.inner.draw(frame);
        if matches!(outcome, FrameOutcome::Presented(_)) {
            self.presenter.note_present();
        }
        let violations: Vec<String> = self
            .inner
            .violations()
            .iter()
            .map(|violation| format!("{violation:?}"))
            .collect();
        let drawn = Drawn {
            frame: frame.id,
            clock: frame.clock,
            full: frame.damage.draw.is_full(),
            shifts: frame.shifts.len(),
            outcome,
        };
        self.probe.update(|state| {
            state.draws.push(drawn);
            state.violations = violations;
        });
        outcome
    }

    fn present_held(&mut self) -> FrameOutcome {
        let outcome = self.inner.present_held();
        self.probe.update(|state| state.held.push(outcome));
        outcome
    }

    fn presenter(&mut self) -> Option<&mut dyn Presenter> {
        Some(&mut self.presenter)
    }

    fn memory(&self) -> MemoryReport {
        self.inner.memory()
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.probe
            .update(|state| state.dropped_on = Some(thread::current().id()));
    }
}
