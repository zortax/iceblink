//! Retiring arena ranges only once nothing in flight reads them.

use std::collections::VecDeque;
use std::ops::Range;
use std::sync::mpsc;

use crate::buffer::chunks::arena::Arena;
use crate::gpu::device::Gpu;

/// Retired ranges, each with its arena.
type RetiredRanges = Vec<(usize, Range<u32>)>;

/// Ranges freed by a frame, returned to their arenas once the device finished the frame.
#[derive(Debug)]
pub(crate) struct RetireLedger {
    /// The last submission ranges were tied to.
    seq: u64,
    /// The ranges retired since the last submission.
    current: RetiredRanges,
    /// The ranges of each submission in flight, oldest first.
    pending: VecDeque<(u64, RetiredRanges)>,
    sender: mpsc::Sender<u64>,
    receiver: mpsc::Receiver<u64>,
}

impl RetireLedger {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            seq: 0,
            current: Vec::new(),
            pending: VecDeque::new(),
            sender,
            receiver,
        }
    }

    /// Retires `range` of arena `lane`.
    pub(crate) fn retire(&mut self, lane: usize, range: Range<u32>) {
        if !range.is_empty() {
            self.current.push((lane, range));
        }
    }

    /// Ties the ranges retired since the last submission to the submission just made.
    pub(crate) fn submitted(&mut self, gpu: &Gpu) {
        if self.current.is_empty() {
            return;
        }
        self.seq += 1;
        let seq = self.seq;
        self.pending
            .push_back((seq, core::mem::take(&mut self.current)));
        let sender = self.sender.clone();
        gpu.queue().on_submitted_work_done(move || {
            let _ = sender.send(seq);
        });
    }

    /// Returns the ranges of completed submissions to `arenas`.
    pub(crate) fn reclaim(&mut self, arenas: &mut [Arena]) {
        let mut completed = 0;
        while let Ok(seq) = self.receiver.try_recv() {
            completed = completed.max(seq);
        }
        if completed == 0 {
            return;
        }
        while let Some((seq, _)) = self.pending.front() {
            if *seq > completed {
                break;
            }
            let (_, ranges) = self.pending.pop_front().expect("front just answered");
            for (lane, range) in ranges {
                arenas[lane].free(range);
            }
        }
    }

    /// Forgets every retired range.
    pub(crate) fn forget(&mut self) {
        self.current.clear();
        self.pending.clear();
        while self.receiver.try_recv().is_ok() {}
        self.seq = 0;
    }
}
