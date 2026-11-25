//! A gate that holds a renderer factory until a case opens it.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, ThreadId};
use std::time::Duration;

use iceblink::{CompositorClient, OutputConfig, OutputId, RendererFactory};

/// How long a held factory waits for its gate before it gives up.
const HELD_FOR: Duration = Duration::from_secs(5);

/// A gate that holds a renderer factory until the case opens it.
///
/// A factory that runs on the case's own thread runs immediately.
pub(super) struct Gate {
    state: Arc<(Mutex<GateState>, Condvar)>,
    case: ThreadId,
}

#[derive(Default)]
struct GateState {
    open: bool,
    gave_up: bool,
}

impl Gate {
    pub(super) fn new() -> Self {
        Self {
            state: Arc::default(),
            case: thread::current().id(),
        }
    }

    /// Creates an output whose factory `factory` runs once the gate opens.
    pub(super) fn create(
        &self,
        driver: &mut dyn CompositorClient,
        factory: RendererFactory,
    ) -> OutputId {
        let state = Arc::clone(&self.state);
        let case = self.case;
        let config = OutputConfig {
            size: [200, 200],
            scale: 1.0,
            opaque: true,
        };
        driver.create_output(
            config,
            Box::new(move || {
                // Nothing could open the gate while the case's own thread waits on it.
                if thread::current().id() != case {
                    let (lock, opened) = &*state;
                    let held = lock.lock().unwrap();
                    let (mut held, wait) = opened
                        .wait_timeout_while(held, HELD_FOR, |held| !held.open)
                        .unwrap();
                    held.gave_up = wait.timed_out();
                }
                factory()
            }),
        )
    }

    pub(super) fn open(&self) {
        let (lock, opened) = &*self.state;
        lock.lock().unwrap().open = true;
        opened.notify_all();
    }

    /// Reports whether a held factory stopped waiting before the gate opened.
    pub(super) fn gave_up(&self) -> bool {
        self.state.0.lock().unwrap().gave_up
    }
}
