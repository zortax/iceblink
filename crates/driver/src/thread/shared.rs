//! State the driver thread publishes for client handles to read without a round trip.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::ThreadId;

use iceblink::{
    ClientId, CommitId, CompositorCapabilities, CompositorMemory, DriverStats, FontRegistry,
    IdSpace, OutputId, OutputStatus, Transaction,
};

use crate::{Mailbox, Pacing};

/// How many clients the driver keeps registered in reserve.
pub(crate) const RESERVE: usize = 2;

/// The driver's published state.
pub(crate) struct Shared {
    pub(crate) capabilities: CompositorCapabilities,
    pub(crate) fonts: Arc<dyn FontRegistry>,
    pub(crate) pacing: Pacing,
    driver: OnceLock<ThreadId>,
    clients: Mutex<Clients>,
    snapshot: Mutex<Snapshot>,
    next_commit: AtomicU64,
}

/// The registered clients, their event queues, and the clients held in reserve.
#[derive(Default)]
pub(crate) struct Clients {
    /// Clients the driver registered ahead, oldest first, which registrations take.
    pub(crate) reserve: Vec<(ClientId, IdSpace)>,
    held: HashMap<ClientId, Held>,
    pub(crate) mailbox: Mailbox,
    /// Whether the driver has stopped applying calls.
    pub(crate) stopped: bool,
}

/// A registered client's id space and spare transaction.
struct Held {
    space: IdSpace,
    /// A transaction with the buffers of the client's last applied commit.
    spare: Option<Transaction>,
}

/// The newest snapshot of the driver, the compositor and every output.
#[derive(Default)]
pub(crate) struct Snapshot {
    pub(crate) stats: DriverStats,
    pub(crate) memory: CompositorMemory,
    /// When `memory` was taken, on the frame clock, or zero to renew it at the next publication.
    pub(crate) memory_ns: u64,
    pub(crate) extra: Vec<(&'static str, u64)>,
    pub(crate) outputs: Vec<PublishedOutput>,
}

/// The newest snapshot of one output.
pub(crate) struct PublishedOutput {
    pub(crate) id: OutputId,
    pub(crate) status: OutputStatus,
    pub(crate) extra: Vec<(&'static str, u64)>,
}

impl Shared {
    pub(crate) fn new(
        capabilities: CompositorCapabilities,
        fonts: Arc<dyn FontRegistry>,
        pacing: Pacing,
    ) -> Self {
        Self {
            capabilities,
            fonts,
            pacing,
            driver: OnceLock::new(),
            clients: Mutex::new(Clients::default()),
            snapshot: Mutex::new(Snapshot::default()),
            next_commit: AtomicU64::new(0),
        }
    }

    pub(crate) fn set_driver(&self, thread: ThreadId) {
        let _ = self.driver.set(thread);
    }

    /// Reports whether the calling thread is the driver thread.
    pub(crate) fn on_driver(&self) -> bool {
        self.driver.get() == Some(&std::thread::current().id())
    }

    pub(crate) fn next_commit(&self) -> CommitId {
        CommitId(self.next_commit.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn clients(&self) -> MutexGuard<'_, Clients> {
        lock(&self.clients)
    }

    pub(crate) fn snapshot(&self) -> MutexGuard<'_, Snapshot> {
        lock(&self.snapshot)
    }
}

impl Clients {
    /// Adds `client` with an empty event queue.
    pub(crate) fn add(&mut self, client: ClientId, space: IdSpace) {
        self.held.insert(client, Held { space, spare: None });
        self.mailbox.open(client);
    }

    /// Removes `client`, its event queue and its waker.
    pub(crate) fn remove(&mut self, client: ClientId) {
        self.held.remove(&client);
        self.mailbox.close(client);
    }

    /// Returns an empty transaction for `client`, with the spare buffers if any, or `None` if
    /// the client is not registered.
    pub(crate) fn transaction(&mut self, client: ClientId) -> Option<Transaction> {
        let held = self.held.get_mut(&client)?;
        Some(
            held.spare
                .take()
                .unwrap_or_else(|| Transaction::new(held.space.clone())),
        )
    }

    /// Keeps `spare` as the next transaction of its client, if the client is registered.
    pub(crate) fn put_spare(&mut self, spare: Transaction) {
        if let Some(held) = self.held.get_mut(&spare.client()) {
            held.spare = Some(spare);
        }
    }
}

// Every write is a single assignment, insert or append, so a panic on another thread cannot leave
// the state half-written.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
