//! An event waker that reports to the test thread.

use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use iceblink::ClientId;
use iceblink::CompositorClient;
use iceblink_driver::thread::ClientHandle;

/// How long a test waits for the waker before it fails.
const PATIENCE: Duration = Duration::from_secs(20);

/// The wakes of a client's waker, each with the name of the thread it ran on.
pub(crate) struct Wakes(mpsc::Receiver<Option<String>>);

impl Wakes {
    /// Sets the waker of `client`.
    pub(crate) fn of(handle: &mut ClientHandle, client: ClientId) -> Self {
        let (woke, woken) = mpsc::channel();
        let woke = Mutex::new(woke);
        let waker = Arc::new(move || {
            let name = thread::current().name().map(str::to_owned);
            let _ = woke.lock().unwrap().send(name);
        });
        handle.set_waker(client, Some(waker));
        Self(woken)
    }

    /// Waits for the next wake, and returns the name of the thread it ran on.
    pub(crate) fn next(&self) -> Option<String> {
        self.0.recv_timeout(PATIENCE).expect("the waker runs")
    }
}
