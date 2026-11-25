//! Revisions of lowered chunks, and the log of revisions no longer referenced.

/// The source of revision numbers and the log of retired revisions.
#[derive(Debug)]
pub(crate) struct Revisions {
    next: u64,
    /// Retired revisions, each with its position in the log.
    log: Vec<(u64, u64)>,
    /// The position the next retirement gets.
    position: u64,
}

impl Default for Revisions {
    fn default() -> Self {
        Self {
            next: 1,
            log: Vec::new(),
            position: 1,
        }
    }
}

impl Revisions {
    /// Returns a revision number no lowered chunk has had.
    pub(crate) fn mint(&mut self) -> u64 {
        let revision = self.next;
        self.next += 1;
        revision
    }

    /// Logs that `revision` will not be referenced again.
    pub(crate) fn retire(&mut self, revision: u64) {
        self.log.push((self.position, revision));
        self.position += 1;
    }

    /// Returns the log position the next retirement gets.
    pub(crate) fn position(&self) -> u64 {
        self.position
    }

    /// Calls `retired` with every revision retired at or after log position `from`.
    pub(crate) fn since(&self, from: u64, mut retired: impl FnMut(u64)) {
        let start = self.log.partition_point(|&(position, _)| position < from);
        for &(_, revision) in &self.log[start..] {
            retired(revision);
        }
    }

    /// Drops the log entries before position `settled`, which every output has considered.
    pub(crate) fn settle(&mut self, settled: u64) {
        let keep = self
            .log
            .partition_point(|&(position, _)| position < settled);
        self.log.drain(..keep);
    }
}
