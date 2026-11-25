//! The answers of a driver that stopped.

use std::panic::{AssertUnwindSafe, catch_unwind};

use iceblink::{ClientEvent, CommitError, RejectReason, ScrollRouting};

use crate::Letter;
use crate::thread::command::Command;
use crate::thread::driver::Host;

impl Host {
    /// Answers the calls still queued as a stopped driver does, and marks the driver stopped, so
    /// handles answer every later call themselves.
    pub(super) fn answer_stopped(&mut self) {
        let mut letters = Vec::new();
        let mut clients = self.shared.clients();
        clients.stopped = true;
        for command in self.commands.try_iter() {
            if let Some(letter) = stopped_answer(command) {
                letters.push(letter);
            }
        }
        clients.mailbox.deliver(&mut letters, &mut self.wake);
        drop(clients);
        for waker in self.wake.drain(..) {
            // The thread may be unwinding already, and a second panic would abort.
            let _ = catch_unwind(AssertUnwindSafe(|| waker()));
        }
    }
}

/// Returns the event that answers `command` when the driver stops before applying it, if the
/// command is answered by an event.
pub(crate) fn stopped_answer(command: Command) -> Option<Letter> {
    match command {
        Command::Commit(commit, transaction) => {
            let error = CommitError {
                reason: RejectReason::Stopped,
                command: None,
            };
            Some(Letter {
                client: transaction.client(),
                event: ClientEvent::Rejected { commit, error },
                wakes: true,
            })
        }
        Command::ScrollInput {
            client,
            output,
            gesture,
            predicted,
        } if predicted != ScrollRouting::Unhandled => Some(Letter {
            client,
            event: ClientEvent::Rerouted {
                output,
                gesture,
                predicted,
                routing: ScrollRouting::Unhandled,
            },
            wakes: true,
        }),
        _ => None,
    }
}
