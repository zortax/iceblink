//! Groups in a frame.

use std::sync::Arc;

use crate::frame::FrameFilter;
use crate::node::BlendMode;

/// Content drawn into a target of its own and composited as one.
///
/// The group's content is drawn into a cleared target. The filters then apply in sequence, the
/// result is multiplied by `opacity` and by the coverage of `clip`, and it is blended onto the
/// target beneath with `blend`.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameGroup {
    /// The written region, as a device `[x, y, width, height]` inside the output, or inside the
    /// `source` of the enclosing group.
    pub bounds: [f32; 4],
    /// The region the content is drawn over before the filters, as a device
    /// `[x, y, width, height]`: the content's ink grown by the read extent of `filters`.
    ///
    /// It contains `bounds`, and content outside it is transparent.
    pub source: [f32; 4],
    /// The frame clip slot applied when the group is composited, or `FRAME_NO_CLIP`.
    pub clip: u32,
    /// A multiplier on the result, clamped to `[0, 1]`.
    pub opacity: f32,
    /// How the result is blended onto the target beneath.
    pub blend: BlendMode,
    /// The filter chain.
    pub filters: Arc<[FrameFilter]>,
}
