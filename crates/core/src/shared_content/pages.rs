//! Atlas page operations, sent to the renderer of every output.

use iceblink::PageOp;
use rustc_hash::FxHashSet;

/// The page operations the content cache made, kept until the renderer of every output received
/// them.
///
/// Each operation has a sequence number, counted from zero over the log's lifetime.
#[derive(Debug, Default)]
pub(crate) struct PageLog {
    ops: Vec<PageOp>,
    /// The sequence number of `ops[0]`.
    start: u64,
    /// The pages that exist.
    live: FxHashSet<u32>,
}

/// The pages one renderer holds, and how far it has read the log.
#[derive(Debug, Default)]
pub(crate) struct PageCursor {
    /// The sequence number of the next operation the renderer has not received.
    pub(crate) next: u64,
    /// The pages the renderer was told to create and not yet to destroy.
    pub(crate) held: FxHashSet<u32>,
    /// Whether the renderer holds no page, so it is sent every page that exists in place of the
    /// log.
    pub(crate) replay: bool,
}

impl PageCursor {
    /// Returns the cursor of a renderer that holds no page.
    pub(crate) fn replaying() -> Self {
        Self {
            replay: true,
            ..Self::default()
        }
    }
}

impl PageLog {
    /// Appends operations, in the order the renderer applies them.
    pub(crate) fn extend(&mut self, ops: impl IntoIterator<Item = PageOp>) {
        for op in ops {
            match &op {
                PageOp::Create { page, .. } => {
                    self.live.insert(*page);
                }
                PageOp::Destroy { page } => {
                    self.live.remove(page);
                }
                PageOp::Upload { .. } => {}
            }
            self.ops.push(op);
        }
    }

    /// Returns the sequence number the next operation gets.
    pub(crate) fn end(&self) -> u64 {
        self.start + self.ops.len() as u64
    }

    /// Moves `cursor` to the end of the log, for a renderer that was given every page that
    /// exists.
    pub(crate) fn catch_up(&self, cursor: &mut PageCursor) {
        cursor.next = self.end();
        cursor.held = self.live.clone();
        cursor.replay = false;
    }

    /// Appends to `out` the operations `cursor` has not received, and moves the cursor to the
    /// end.
    ///
    /// Uploads and destructions of pages the renderer does not hold are left out.
    pub(crate) fn send(&self, cursor: &mut PageCursor, out: &mut Vec<PageOp>) {
        let from = cursor.next.saturating_sub(self.start) as usize;
        for op in self.ops.iter().skip(from) {
            let keep = match op {
                PageOp::Create { page, .. } => cursor.held.insert(*page),
                PageOp::Upload { page, .. } => cursor.held.contains(page),
                PageOp::Destroy { page } => cursor.held.remove(page),
            };
            if keep {
                out.push(op.clone());
            }
        }
        cursor.next = self.end();
    }

    /// Drops the operations before sequence number `received`, which every renderer has.
    pub(crate) fn settle(&mut self, received: u64) {
        let drop = received
            .saturating_sub(self.start)
            .min(self.ops.len() as u64) as usize;
        if drop == 0 {
            return;
        }

        self.ops.drain(..drop);
        self.start += drop as u64;
    }
}
