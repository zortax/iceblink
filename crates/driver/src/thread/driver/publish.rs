//! What the driver thread publishes after each batch.

use crate::now_ns;
use crate::thread::driver::Host;
use crate::thread::shared::PublishedOutput;

/// How old the published memory of the compositor may grow before a publication renews it.
const MEMORY_AGE_NS: u64 = 100_000_000;

impl Host {
    /// Delivers the queued letters and recycled transactions, publishes the snapshot, and then
    /// calls the wakers the letters owe.
    pub(super) fn publish(&mut self) {
        {
            let mut clients = self.shared.clients();
            clients
                .mailbox
                .deliver(self.driver.letters(), &mut self.wake);
            for spare in self.spares.drain(..) {
                clients.put_spare(spare);
            }
        }
        self.publish_snapshot();
        for waker in self.wake.drain(..) {
            waker();
        }
    }

    /// Publishes a snapshot of the driver, the compositor and every output.
    fn publish_snapshot(&self) {
        let mut snapshot = self.shared.snapshot();
        let snapshot = &mut *snapshot;
        snapshot.stats = self.driver.stats();
        let now = now_ns();
        if snapshot.memory_ns == 0 || now >= snapshot.memory_ns + MEMORY_AGE_NS {
            snapshot.memory = self.driver.memory();
            snapshot.memory_ns = now.max(1);
        }

        snapshot.extra.clear();
        self.driver
            .extra_stats(&mut |name, value| snapshot.extra.push((name, value)));

        let mut count = 0;
        for id in self.driver.outputs() {
            let Some(status) = self.driver.output_status(id) else {
                continue;
            };
            if count == snapshot.outputs.len() {
                snapshot.outputs.push(PublishedOutput {
                    id,
                    status,
                    extra: Vec::new(),
                });
            }
            let published = &mut snapshot.outputs[count];
            published.id = id;
            published.status = status;
            published.extra.clear();
            self.driver
                .output_extra_stats(id, &mut |name, value| published.extra.push((name, value)));
            count += 1;
        }
        snapshot.outputs.truncate(count);
    }
}
