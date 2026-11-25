//! The damage hint of a chunk.

use crate::geometry::LocalRect;
use crate::transaction::recorder::ChunkRecorder;

impl ChunkRecorder<'_> {
    /// Narrows the damage a re-recording of this chunk adds to `within`, a rectangle in the
    /// chunk's space.
    ///
    /// A second call widens the hint to the bounding box of both rectangles.
    pub fn hint(&mut self, within: LocalRect) {
        self.chunk.hint = Some(match self.chunk.hint {
            Some(hint) => union(hint, within),
            None => within,
        });
    }
}

/// Returns the bounding box of `a` and `b`.
fn union(a: LocalRect, b: LocalRect) -> LocalRect {
    let x0 = a[0].min(b[0]);
    let y0 = a[1].min(b[1]);
    let x1 = (a[0] + a[2]).max(b[0] + b[2]);
    let y1 = (a[1] + a[3]).max(b[1] + b[3]);
    [x0, y0, x1 - x0, y1 - y0]
}
