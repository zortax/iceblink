//! The client's side of a threaded compositor.

mod client;

use std::sync::Arc;
use std::sync::mpsc::Sender;

use crate::thread::command::Command;
use crate::thread::driver::stopped_answer;
use crate::thread::shared::Shared;

/// A client's connection to a threaded compositor, usable from any thread.
///
/// Every clone queues its calls on one channel to the driver thread. The capabilities,
/// transactions, events and snapshots are answered on the calling thread.
///
/// Once the driver has stopped, queued calls are dropped. A commit is then answered with a
/// `Rejected` event for `RejectReason::Stopped`, and a gesture predicted to be handled with a
/// `Rerouted` event to `ScrollRouting::Unhandled`.
#[derive(Clone)]
pub struct ClientHandle {
    commands: Sender<Command>,
    shared: Arc<Shared>,
}

impl ClientHandle {
    pub(crate) fn new(commands: Sender<Command>, shared: Arc<Shared>) -> Self {
        Self { commands, shared }
    }

    /// Queues `command`, and reports whether the driver is still running to apply it.
    fn send(&self, command: Command) -> bool {
        self.commands.send(command).is_ok()
    }

    /// Queues `command`, whose answer the caller waits for, and reports whether the driver is
    /// still running to apply it.
    ///
    /// # Panics
    ///
    /// Panics on the driver thread.
    fn ask(&self, command: Command) -> bool {
        assert!(
            !self.shared.on_driver(),
            "a blocking call of a compositor handle on the compositor's own thread never returns"
        );
        self.send(command)
    }

    /// Queues `command`, whose answer is an event, or answers it as a stopped driver does.
    ///
    /// The clients' lock orders the call against the driver stopping, so every such call is
    /// either applied by the driver or answered here.
    fn send_answered(&self, command: Command) {
        let mut clients = self.shared.clients();
        let command = if clients.stopped {
            command
        } else {
            match self.commands.send(command) {
                Ok(()) => return,
                Err(failed) => failed.0,
            }
        };
        let Some(letter) = stopped_answer(command) else {
            return;
        };
        let mut wake = Vec::new();
        clients.mailbox.deliver(&mut vec![letter], &mut wake);
        drop(clients);
        for waker in wake {
            waker();
        }
    }
}
