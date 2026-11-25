//! What the fakes on the driver thread report to the test thread.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, ThreadId};
use std::time::Duration;

use iceblink::{FrameClock, FrameId, FrameOutcome};
use iceblink_cpu::Pixmap;

/// How long a test waits for the driver before it fails.
const PATIENCE: Duration = Duration::from_secs(20);

/// One frame a fake renderer drew.
#[derive(Clone, Debug)]
pub(crate) struct Drawn {
    pub(crate) frame: FrameId,
    pub(crate) clock: FrameClock,
    /// Whether the frame redraws the whole output.
    pub(crate) full: bool,
    pub(crate) shifts: usize,
    pub(crate) outcome: FrameOutcome,
}

/// State a fake renderer or presenter shares with the test.
#[derive(Default)]
pub(crate) struct Probe {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) draws: Vec<Drawn>,
    /// Outcomes the next frames answer with, in place of drawing.
    pub(crate) script: VecDeque<FrameOutcome>,
    pub(crate) violations: Vec<String>,
    pub(crate) built_on: Option<ThreadId>,
    pub(crate) dropped_on: Option<ThreadId>,
    /// The image presented last.
    pub(crate) image: Option<Pixmap>,
    pub(crate) presents: usize,
    /// The outcomes of `present_held` calls, in order.
    pub(crate) held: Vec<FrameOutcome>,
}

impl Probe {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// Changes the state and wakes the waiting test.
    pub(crate) fn update(&self, change: impl FnOnce(&mut State)) {
        change(&mut self.state());
        self.changed.notify_all();
    }

    /// Records that a fake was built on the calling thread.
    pub(crate) fn built_here(&self) {
        self.update(|state| state.built_on = Some(thread::current().id()));
    }

    /// Waits until `done` holds for the state, and returns what it returns.
    ///
    /// # Panics
    ///
    /// Panics if it does not hold within the test's patience.
    pub(crate) fn wait<T>(&self, mut done: impl FnMut(&State) -> Option<T>) -> T {
        let state = self.state();
        let (state, timeout) = self
            .changed
            .wait_timeout_while(state, PATIENCE, |state| done(state).is_none())
            .unwrap();
        assert!(!timeout.timed_out(), "the driver did not get there");
        done(&state).unwrap()
    }

    /// Waits until `count` frames were drawn, and returns them.
    pub(crate) fn wait_draws(&self, count: usize) -> Vec<Drawn> {
        self.wait(|state| (state.draws.len() >= count).then(|| state.draws.clone()))
    }

    pub(crate) fn draws(&self) -> usize {
        self.state().draws.len()
    }
}
