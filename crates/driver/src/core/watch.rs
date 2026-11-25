//! A scroll watch and the ranges it still has armed.

use iceblink::smallvec::SmallVec;
use iceblink::{ScrollWatch, WatchCause};

/// A watch of one scroll node, with the ranges still armed.
#[derive(Debug, Default)]
pub(crate) struct Watched {
    watch: ScrollWatch,
    /// Whether each range can still fire.
    armed: SmallVec<[bool; 2]>,
}

impl Watched {
    /// Returns a watch with every range of `watch` armed.
    pub(crate) fn new(watch: ScrollWatch) -> Self {
        Self {
            armed: watch.ranges.iter().map(|_| true).collect(),
            watch,
        }
    }

    /// Checks a scroll event at `offset` and returns what it fired, in the order range, stride,
    /// settle.
    ///
    /// The node's previous event was at `last`, its motion step is `step`, and the event settles
    /// the node when `settled` holds.
    pub(crate) fn check(
        &mut self,
        offset: [f32; 2],
        last: Option<[f32; 2]>,
        step: [f32; 2],
        settled: bool,
    ) -> SmallVec<[WatchCause; 2]> {
        let mut fired = SmallVec::new();
        for (index, range) in self.watch.ranges.iter().enumerate() {
            if !self.armed[index] {
                continue;
            }
            let ahead = [
                offset[0] + range.ahead * step[0],
                offset[1] + range.ahead * step[1],
            ];
            if !range.contains(offset) || !range.contains(ahead) {
                self.armed[index] = false;
                fired.push(WatchCause::Left(index));
            }
        }
        if let Some(last) = last
            && crossed(last, offset, self.watch.stride)
        {
            fired.push(WatchCause::Crossed);
        }
        if settled && self.watch.settle {
            fired.push(WatchCause::Settled);
        }
        fired
    }
}

/// Reports whether moving from `from` to `to` passes a multiple of `stride` on an axis with a
/// nonzero stride.
fn crossed(from: [f32; 2], to: [f32; 2], stride: [f32; 2]) -> bool {
    (0..2).any(|axis| {
        stride[axis] > 0.0
            && (from[axis] / stride[axis]).floor() != (to[axis] / stride[axis]).floor()
    })
}
