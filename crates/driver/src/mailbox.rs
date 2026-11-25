//! The event queues and wakers of a driver's clients.

use std::collections::HashMap;

use iceblink::{ClientEvent, ClientId, CompositorEvent, EventWaker, ScrollPhase};

/// An event a driver addressed to one client, and whether it wakes the client.
#[derive(Clone, Debug, PartialEq)]
pub struct Letter {
    /// The client the event is queued for.
    pub client: ClientId,
    /// The event to queue.
    pub event: ClientEvent,
    /// Whether the event wakes the client.
    pub wakes: bool,
}

/// The event queues and wakers of a driver's clients.
///
/// A queued scroll event of a node is replaced by a newer one of the same node, unless it reports
/// that the node settled. A client is woken at most once between two drains of its queue, and
/// once more after each change of its waker.
#[derive(Default)]
pub struct Mailbox {
    inboxes: HashMap<ClientId, Inbox>,
}

#[derive(Default)]
struct Inbox {
    events: Vec<ClientEvent>,
    waker: Option<EventWaker>,
    /// Whether the client was woken since it last drained its queue.
    woken: bool,
}

impl Mailbox {
    /// Adds an empty queue for `client`.
    pub fn open(&mut self, client: ClientId) {
        self.inboxes.entry(client).or_default();
    }

    /// Reports whether `client` has a queue.
    pub fn contains(&self, client: ClientId) -> bool {
        self.inboxes.contains_key(&client)
    }

    /// Removes the queue and the waker of `client`.
    pub fn close(&mut self, client: ClientId) {
        self.inboxes.remove(&client);
    }

    /// Sets the waker of `client`, or removes it with `None`.
    ///
    /// The next waking event calls the new waker, whatever was queued before.
    pub fn set_waker(&mut self, client: ClientId, waker: Option<EventWaker>) {
        if let Some(inbox) = self.inboxes.get_mut(&client) {
            inbox.waker = waker;
            inbox.woken = false;
        }
    }

    /// Moves the queued events of `client` into `out`, in the order they were queued.
    pub fn drain(&mut self, client: ClientId, out: &mut Vec<ClientEvent>) {
        if let Some(inbox) = self.inboxes.get_mut(&client) {
            out.append(&mut inbox.events);
            inbox.woken = false;
        }
    }

    /// Queues every letter of `letters` and empties it, then adds the wakers of the clients the
    /// letters wake to `wake`.
    ///
    /// A letter to a client without a queue is dropped.
    pub fn deliver(&mut self, letters: &mut Vec<Letter>, wake: &mut Vec<EventWaker>) {
        for letter in letters.drain(..) {
            let Some(inbox) = self.inboxes.get_mut(&letter.client) else {
                continue;
            };
            inbox.push(letter.event);
            if letter.wakes && !inbox.woken {
                inbox.woken = true;
                wake.extend(inbox.waker.clone());
            }
        }
    }
}

impl Inbox {
    fn push(&mut self, event: ClientEvent) {
        if let ClientEvent::Compositor(CompositorEvent::Scroll { node, .. }) = &event {
            let newest = self.events.iter().rposition(|queued| {
                matches!(
                    queued,
                    ClientEvent::Compositor(CompositorEvent::Scroll { node: queued, .. })
                        if queued == node
                )
            });
            if let Some(index) = newest
                && let ClientEvent::Compositor(CompositorEvent::Scroll { phase, .. }) =
                    &self.events[index]
                && *phase != ScrollPhase::Settled
            {
                self.events.remove(index);
            }
        }
        self.events.push(event);
    }
}
