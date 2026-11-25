//! The offset that moves a thumb node along its track.

use iceblink::{ScrollNode, ThumbAnchor};

use crate::naive::space::{current_offset, largest_offset};

/// Returns the thumb translation of `anchor` under the current offset of `scroll`.
pub(super) fn offset(anchor: &ThumbAnchor, scroll: &ScrollNode) -> [f32; 2] {
    let offset = current_offset(scroll);
    let largest = largest_offset(scroll);
    [0, 1].map(|axis| {
        if largest[axis] > 0.0 {
            anchor.track[axis] * offset[axis] / largest[axis]
        } else {
            0.0
        }
    })
}
