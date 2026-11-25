//! The driver's calls for clients: registration, commits, gestures and watches.

use iceblink::smallvec::SmallVec;
use iceblink::{
    ClientEvent, ClientId, CommitId, HitResult, IdSpace, OutputId, ScrollGesture, ScrollId,
    ScrollRouting, ScrollWatch, Transaction,
};

use crate::core::Driver;
use crate::core::events::Watches;
use crate::core::watch::Watched;

impl Driver {
    /// Adds a client and returns its id and the id space its transactions mint from.
    pub fn register_client(&mut self) -> (ClientId, IdSpace) {
        let (client, space) = self.compositor.register_client();
        self.clients.insert(client, Watches::default());
        (client, space)
    }

    /// Removes `client`, everything it created and its watches.
    pub fn unregister_client(&mut self, client: ClientId) {
        self.compositor.unregister_client(client);
        self.clients.remove(&client);
        for output in &mut self.outputs {
            output.watchers.retain(|watcher| *watcher != client);
            output.shown.retain(|watcher| *watcher != client);
            output.presenter.retain(|watcher| *watcher != client);
        }
        self.route_events();

        let now = self.now();
        self.ask_all(now);
    }

    /// Returns an empty transaction for `client`, with buffers recycled from earlier commits.
    pub fn transaction(&mut self, client: ClientId) -> Transaction {
        self.compositor.transaction(client)
    }

    /// Applies `transaction`, and addresses a `Committed` or `Rejected` event to its client.
    ///
    /// An applied transaction applies the configurations that wait for a commit and starts the
    /// frames of every output.
    pub fn commit(&mut self, commit: CommitId, transaction: Transaction) {
        let client = transaction.client();
        let event = match self.compositor.commit(transaction) {
            Ok(report) => ClientEvent::Committed { commit, report },
            Err(error) => ClientEvent::Rejected { commit, error },
        };
        let rejected = matches!(event, ClientEvent::Rejected { .. });
        self.send(client, event, rejected);
        self.route_events();

        // A commit can change what any output shows.
        let now = self.now();
        if rejected {
            self.ask_all(now);
        } else {
            self.apply_deferred(None);
            for output in &mut self.outputs {
                output.start(now);
            }
        }
    }

    /// Routes a gesture `client` handed for `output`, and addresses a `Rerouted` event to the
    /// client when the routing differs from `predicted`.
    ///
    /// A gesture a scroll node takes moves content with the compositor's motion, so its frame
    /// keeps the paced latch on an idle output too.
    pub fn scroll_input(
        &mut self,
        client: ClientId,
        output: OutputId,
        gesture: ScrollGesture,
        predicted: ScrollRouting,
    ) {
        let routing = self.compositor.scroll_input(output, gesture);
        if routing != predicted {
            let event = ClientEvent::Rerouted {
                output,
                gesture,
                predicted,
                routing,
            };
            self.send(client, event, true);
        }
        self.route_events();

        let now = self.now();
        if let Some(output) = self.output(output) {
            match routing {
                ScrollRouting::Handled { .. } => output.ask_paced(now),
                ScrollRouting::Unhandled => output.ask(now),
            }
        }
    }

    /// Returns every item under a device point of `output`, front to back.
    pub fn hit_test(&self, output: OutputId, point: [f32; 2]) -> SmallVec<[HitResult; 4]> {
        self.compositor.hit_test(output, point)
    }

    /// Sets the watch of a scroll node of `client`, or removes it with `None`.
    pub fn watch_scroll(&mut self, client: ClientId, node: ScrollId, watch: Option<ScrollWatch>) {
        let Some(watches) = self.clients.get_mut(&client) else {
            return;
        };
        match watch {
            Some(watch) => {
                watches.scroll.insert(node, Watched::new(watch));
            }
            None => {
                watches.scroll.remove(&node);
            }
        }
    }

    /// Sets whether the animation events of `client` wake it.
    pub fn watch_animations(&mut self, client: ClientId, on: bool) {
        if let Some(watches) = self.clients.get_mut(&client) {
            watches.animations = on;
        }
    }
}
