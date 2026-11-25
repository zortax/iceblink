//! The driver thread, which applies queued calls between frames, runs the frames of every output
//! and publishes what changed.

mod apply;
mod publish;
mod stop;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use iceblink::{Compositor, EventWaker, Transaction};

use crate::thread::command::Command;
use crate::thread::shared::{RESERVE, Shared};
use crate::{Driver, DriverOptions, Pacing, now_ns};

pub(crate) use self::stop::stopped_answer;

/// Runs `compositor` until a `Shutdown` command arrives or every sender is dropped.
///
/// The driver registers its reserve clients and publishes them before it applies any call.
pub(crate) fn run(
    compositor: Box<dyn Compositor>,
    options: DriverOptions,
    commands: Receiver<Command>,
    shared: Arc<Shared>,
) {
    let mut host = Host {
        driver: Driver::new(compositor, options),
        commands,
        shared,
        spares: Vec::new(),
        wake: Vec::new(),
    };
    for _ in 0..RESERVE {
        host.reserve();
    }
    host.publish();
    host.run();
}

/// The driver thread's state.
struct Host {
    driver: Driver,
    commands: Receiver<Command>,
    shared: Arc<Shared>,
    /// The transactions recycled since the last publication.
    spares: Vec<Transaction>,
    /// The wakers a publication owes, kept for the allocation.
    wake: Vec<EventWaker>,
}

/// The most commands the driver applies before it publishes.
const BATCH: usize = 64;

/// What the driver does after a command.
#[derive(PartialEq, Eq)]
enum Flow {
    /// It applies the next queued command.
    Continue,
    /// It builds due frames and publishes before it applies the next command.
    Publish,
    /// It stops.
    Stop,
}

impl Host {
    fn run(&mut self) {
        loop {
            let received = match self.due() {
                None => self
                    .commands
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected),
                Some(due) => {
                    let wait = due.saturating_sub(now_ns());
                    self.commands.recv_timeout(Duration::from_nanos(wait))
                }
            };
            match received {
                Ok(command) => {
                    let flow = match self.apply(command) {
                        Flow::Continue => self.apply_queued(),
                        flow => flow,
                    };
                    if flow == Flow::Stop {
                        self.publish();
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }

            self.driver.run_due(now_ns());
            self.publish();
        }
    }

    /// Returns when a paced driver next has work.
    fn due(&self) -> Option<u64> {
        match self.driver.options().pacing {
            Pacing::Paced => self.driver.next_wake(),
            Pacing::Stepped => None,
        }
    }

    /// Applies the commands already queued, until the queue is empty, a frame latches, or a batch
    /// of commands was applied.
    ///
    /// A frame an idle output latched immediately for a command also shows the commands queued
    /// behind that command.
    ///
    /// Returns `Publish` right after a step and `Stop` right after a shutdown.
    fn apply_queued(&mut self) -> Flow {
        // The command `run` received counts as the first of the batch.
        for _ in 1..BATCH {
            if self.driver.latched(now_ns()) {
                break;
            }
            let Ok(command) = self.commands.try_recv() else {
                break;
            };
            match self.apply(command) {
                Flow::Continue => {}
                flow => return flow,
            }
        }
        Flow::Continue
    }

    /// Registers a client for a later registration to take, and publishes it.
    fn reserve(&mut self) {
        let reserve = self.driver.register_client();
        self.shared.clients().reserve.push(reserve);
    }
}

impl Drop for Host {
    // Runs when the driver stops and when its thread panics.
    fn drop(&mut self) {
        self.answer_stopped();
    }
}
