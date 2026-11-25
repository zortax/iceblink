//! A count of frames a client saw, which frame observers wait on.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// How long an observer waits for the client before it gives up.
const PATIENCE: Duration = Duration::from_secs(5);

/// The number of frames a client has seen reported, which a thread can wait on.
#[derive(Default)]
pub(super) struct Seen {
    count: Mutex<usize>,
    changed: Condvar,
}

impl Seen {
    /// Records that the client saw `count` frames.
    pub(super) fn set(&self, count: usize) {
        *self.count.lock().unwrap() = count;
        self.changed.notify_all();
    }

    /// Waits until the client saw `count` frames, and reports whether it did in time.
    pub(super) fn wait_for(&self, count: usize) -> bool {
        let seen = self.count.lock().unwrap();
        let (_seen, waited) = self
            .changed
            .wait_timeout_while(seen, PATIENCE, |seen| *seen < count)
            .unwrap();
        !waited.timed_out()
    }
}
