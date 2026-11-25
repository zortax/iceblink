//! An output's renderer and the frame lifecycle a driver runs for it.

mod anchor;
mod budget;
mod display;
mod draw;
mod feedback;
mod schedule;
mod state;
mod want;

use iceblink::{
    ClientId, Frame, FrameCounters, FrameId, FrameObserver, FrameOutcome, OutputConfig, OutputId,
    OutputStatus, RenderTarget, Renderer, RendererTask, SubpixelOrder,
};

use crate::options::{DriverOptions, Pacing};

use self::budget::Budget;
use self::display::Display;
use self::schedule::Schedule;

pub(crate) use self::state::PresenterState;

/// How many reports of frames that reached the display an output keeps until it reads them.
const REPORTED: usize = 16;

/// An output of the compositor, with the renderer that draws its frames.
pub(crate) struct Output {
    pub(crate) id: OutputId,
    renderer: Box<dyn Renderer>,
    frame: Frame,
    pacing: Pacing,
    pub(crate) schedule: Schedule,
    budget: Budget,
    /// Whether a frame composed and skipped with its damage retired waits to be presented.
    unpresented: bool,
    /// Whether no frame was built since the output was made or configured.
    fresh: bool,
    /// Whether a commit was applied or a frame requested since the output was made. Until then
    /// the output wants no frame.
    started: bool,
    /// The configuration that takes effect with the next commit or frame request.
    deferred: Option<OutputConfig>,
    display: Display,
    subpixel: Option<SubpixelOrder>,
    /// The refresh interval of the output's display, as the client reported it.
    refresh: Option<u64>,
    /// When frames reached the display as the client reported it, oldest first.
    reported: Vec<u64>,
    frames_built: u64,
    /// The clients that get a `Drawn` event for every frame drawn.
    pub(crate) watchers: Vec<ClientId>,
    /// The clients that get a `Shown` event for the next frame presented.
    pub(crate) shown: Vec<ClientId>,
    /// The clients that get a `PresenterChanged` event when the presenter state changes.
    pub(crate) presenter: Vec<ClientId>,
    pub(crate) state: PresenterState,
    observer: Option<FrameObserver>,
    /// The presenter's deadline as last read.
    deadline: Option<u64>,
}

/// What one frame of an output did: a frame built and drawn, a held frame presented, or a frame
/// skipped before it was built.
pub(crate) struct Drawn {
    /// The frame built, or the output's last built frame when none was.
    pub(crate) frame: FrameId,
    pub(crate) counters: FrameCounters,
    pub(crate) outcome: FrameOutcome,
    /// Whether the frame was built and drawn.
    pub(crate) built: bool,
}

impl Output {
    /// Returns output `id` drawn by `renderer` and configured for `config`.
    pub(crate) fn new(
        id: OutputId,
        mut renderer: Box<dyn Renderer>,
        config: OutputConfig,
        options: &DriverOptions,
    ) -> Self {
        renderer.configure(target(config));
        Self {
            id,
            renderer,
            frame: Frame::default(),
            pacing: options.pacing,
            schedule: Schedule::new(options.pacing, options.interval_ns),
            budget: Budget::new(options.budget_ns),
            unpresented: false,
            fresh: true,
            started: false,
            deferred: None,
            display: Display::new(options.interval_ns),
            subpixel: None,
            refresh: None,
            reported: Vec::new(),
            frames_built: 0,
            watchers: Vec::new(),
            shown: Vec::new(),
            presenter: Vec::new(),
            state: PresenterState::default(),
            observer: None,
            deadline: None,
        }
    }

    pub(crate) fn configure(&mut self, config: OutputConfig, now_ns: u64) {
        self.fresh = true;
        self.renderer.configure(target(config));
        self.ask(now_ns);
    }

    /// Keeps `config` until [`take_deferred`](Self::take_deferred), when the output's frames
    /// have started, and reports whether it did.
    ///
    /// A later configuration replaces one that is kept.
    pub(crate) fn defer(&mut self, config: OutputConfig) -> bool {
        if self.started {
            self.deferred = Some(config);
        }
        self.started
    }

    pub(crate) fn take_deferred(&mut self) -> Option<OutputConfig> {
        self.deferred.take()
    }

    pub(crate) fn set_subpixel_order(&mut self, order: Option<SubpixelOrder>, now_ns: u64) {
        self.subpixel = order;
        if let Some(order) = order {
            self.renderer.set_subpixel_order(order);
        }
        self.ask(now_ns);
    }

    /// Takes `refresh_ns` as the refresh interval of the output's display, or forgets it, at
    /// `now_ns`.
    ///
    /// A paced output then plans on the interval of the first source that has one, and moves a
    /// frame already due onto the refreshes when they changed.
    pub(crate) fn set_refresh(&mut self, refresh_ns: Option<u64>, now_ns: u64) {
        self.refresh = refresh_ns;
        // A stepped output takes its interval from the step clocks.
        if self.pacing == Pacing::Paced {
            self.read_feedback(now_ns);
        }
    }

    /// Starts the output's frames and wants one.
    pub(crate) fn start(&mut self, now_ns: u64) {
        self.started = true;
        self.ask(now_ns);
    }

    /// Takes `presented_ns`, when a frame reached the display as the client reported it, at
    /// `now_ns`.
    pub(crate) fn report_presented(&mut self, presented_ns: u64, now_ns: u64) {
        if self.reported.len() == REPORTED {
            self.reported.remove(0);
        }
        self.reported.push(presented_ns);
        // A stepped output never reads the reports.
        if self.pacing == Pacing::Paced {
            self.read_feedback(now_ns);
        }
    }

    /// Returns a snapshot of the output and its renderer.
    pub(crate) fn status(&self) -> OutputStatus {
        OutputStatus {
            caps: self.renderer.capabilities(),
            vector: self.renderer.vector_status(),
            memory: self.renderer.memory(),
            target_pool: self.renderer.target_pool(),
            frames_built: self.frames_built,
            failed: false,
            starved: self.state.starved,
            occluded: self.state.occluded,
        }
    }

    pub(crate) fn set_observer(&mut self, observer: Option<FrameObserver>) {
        self.observer = observer;
    }

    pub(crate) fn run(&mut self, task: RendererTask) {
        task(&mut *self.renderer);
    }

    pub(crate) fn release_cached_targets(&mut self) {
        self.renderer.release_cached_targets();
    }

    pub(crate) fn release_idle_resources(&mut self) {
        self.renderer.release_idle_resources();
    }

    pub(crate) fn extra_stats(&self, out: &mut dyn FnMut(&'static str, u64)) {
        self.renderer.extra_stats(out);
    }
}

fn target(config: OutputConfig) -> RenderTarget {
    RenderTarget {
        size: config.size,
        scale: config.scale,
        opaque: config.opaque,
    }
}
