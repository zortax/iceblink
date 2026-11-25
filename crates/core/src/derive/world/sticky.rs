//! The offset that holds a sticky node inside its scroll port.

use iceblink::StickyAnchor;

use crate::scene::Scene;

/// Returns the translation that holds a sticky node inside its scroll port.
pub(super) fn offset(scene: &Scene, sticky: StickyAnchor) -> [f32; 2] {
    let Some(scroll) = scene.scrolls.at(sticky.scroll.slot()) else {
        return [0.0; 2];
    };
    let port = scroll.node.port;
    let offset = scroll.node.offset;
    let moved = |rect: [f32; 4]| [rect[0] - offset[0], rect[1] - offset[1], rect[2], rect[3]];
    let (bounds, limit) = (moved(sticky.rect), moved(sticky.limit));
    let [top, right, bottom, left] = sticky.inset;
    [
        axis(
            [port[0], port[2]],
            [bounds[0], bounds[2]],
            [limit[0], limit[2]],
            [left, right],
        ),
        axis(
            [port[1], port[3]],
            [bounds[1], bounds[3]],
            [limit[1], limit[3]],
            [top, bottom],
        ),
    ]
}

/// Computes the sticky offset on one axis.
///
/// The port, the sticky box and its limit are each a start and a length, and the insets are the
/// leading and the trailing one, NaN where an edge is free.
fn axis(port: [f32; 2], sticky: [f32; 2], limit: [f32; 2], insets: [f32; 2]) -> f32 {
    let [lead, trail] = insets;
    let mut offset = 0.0_f32;
    if !lead.is_nan() {
        offset = offset.max(port[0] + lead - sticky[0]);
        offset = offset.min(limit[0] + limit[1] - sticky[0] - sticky[1]);
    }
    if !trail.is_nan() {
        offset = offset.min(port[0] + port[1] - trail - sticky[0] - sticky[1]);
        offset = offset.max(limit[0] - sticky[0]);
    }
    offset
}
