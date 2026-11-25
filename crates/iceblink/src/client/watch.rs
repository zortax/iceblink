//! The scroll motion that wakes a client.

use crate::smallvec::SmallVec;

/// The changes of one scroll node's shown offset that wake its client.
///
/// Offsets are in the units of the offsets of `CompositorEvent::Scroll`. A node's motion step is
/// how far its shown offset moved between its last two scroll events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScrollWatch {
    /// The ranges of offsets the client can show without running.
    ///
    /// A range fires once when the offset, or the offset `ahead` motion steps further, leaves
    /// it. It fires again only after the watch is set again.
    pub ranges: SmallVec<[OffsetRange; 2]>,
    /// The spacing of the offsets whose crossing fires the watch, per axis, or zero for none.
    ///
    /// The watch fires whenever the offset on an axis with a nonzero stride moves past a multiple
    /// of the stride.
    pub stride: [f32; 2],
    /// Whether the node coming to rest fires the watch.
    pub settle: bool,
}

/// A range of offsets a scroll watch fires on leaving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffsetRange {
    /// The lowest and highest offset of the range, as `[low, high]` for `x` and then `y`.
    pub bounds: [[f32; 2]; 2],
    /// How many motion steps ahead of the shown offset the range is checked too.
    pub ahead: f32,
}

impl OffsetRange {
    /// Reports whether `offset` lies in the range, bounds included.
    pub fn contains(&self, offset: [f32; 2]) -> bool {
        (0..2).all(|axis| {
            let [low, high] = self.bounds[axis];
            (low..=high).contains(&offset[axis])
        })
    }
}

/// The changes of one output that wake a client.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutputWatch {
    /// Whether the next frame the output presents queues a `Shown` event.
    ///
    /// It fires once, and fires again only after the watch is set again.
    pub shown: bool,
    /// Whether a change of the output's starved or occluded state queues a `PresenterChanged`
    /// event.
    pub presenter: bool,
}
