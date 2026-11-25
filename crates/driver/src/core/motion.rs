//! The motion of a scroll node, from its scroll events.

/// The offset of a scroll node's last scroll event, and how far its offset moved before it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Motion {
    /// The offset of the node's last scroll event.
    last: Option<[f32; 2]>,
    /// How far the offset moved between the node's last two scroll events.
    step: [f32; 2],
}

impl Motion {
    /// Returns how far the offset moved between the node's last two scroll events.
    pub(crate) fn step(&self) -> [f32; 2] {
        self.step
    }

    /// Takes a scroll event at `offset`, which settles the node when `settled` holds, and returns
    /// the offset of the event before it.
    ///
    /// A settle leaves the motion step as it was.
    pub(crate) fn observe(&mut self, offset: [f32; 2], settled: bool) -> Option<[f32; 2]> {
        let last = self.last.replace(offset);
        if let Some(last) = last
            && !settled
        {
            self.step = [offset[0] - last[0], offset[1] - last[1]];
        }
        last
    }
}
