//! The offset that moves a thumb node along its track.

use iceblink::ThumbAnchor;

use crate::scene::Scene;
use crate::tier::scroll::state::range;

/// Returns the translation that places a thumb node on its track at the current offset of its
/// scroll node.
pub(super) fn offset(scene: &Scene, thumb: ThumbAnchor) -> [f32; 2] {
    let Some(scroll) = scene.scrolls.at(thumb.scroll.slot()) else {
        return [0.0; 2];
    };
    let max = range(&scroll.node);
    [0, 1].map(|axis| {
        if max[axis] > 0.0 {
            thumb.track[axis] * (scroll.node.offset[axis] / max[axis]).clamp(0.0, 1.0)
        } else {
            0.0
        }
    })
}
