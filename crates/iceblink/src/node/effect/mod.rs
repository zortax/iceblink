//! Effect nodes.

mod blend;
mod extent;
mod filter;

use std::sync::Arc;

use crate::id::ClipId;

pub use self::blend::BlendMode;
pub use self::extent::read_extent;
pub use self::filter::Filter;

/// The opacity, blending, filters and clip applied to the subtree of every order node that refers
/// to the effect.
///
/// An effect isolates when any of these holds:
///
/// 1. `opacity` is below 1.
/// 2. `blend` is not [`BlendMode::NORMAL`].
/// 3. `filters` is not empty.
/// 4. `isolate` is true.
///
/// An isolating effect draws its subtree into a group. The group's content is drawn into a cleared
/// target in paint order. The filters then apply in sequence, the result is multiplied by the
/// opacity, by `clip` and by the clips of the effects above that do not isolate, up to the nearest
/// enclosing group, and it is blended onto the target beneath with `blend`.
///
/// An effect that does not isolate draws its subtree in place. Its `clip` applies to every item
/// outside nested groups, in addition to the item's own chain, and to the composite of each
/// outermost group nested in the subtree.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectNode {
    /// The clip applied to the subtree.
    pub clip: Option<ClipId>,
    /// The multiplier on the group, clamped to `[0, 1]`.
    pub opacity: f32,
    /// How the group is composited onto the target beneath.
    pub blend: BlendMode,
    /// The filters applied to the group's content, in sequence.
    pub filters: Arc<[Filter]>,
    /// Whether the subtree is drawn into a group whatever the other fields say.
    pub isolate: bool,
}

impl EffectNode {
    /// Reports whether the effect draws its subtree into a group.
    pub fn isolates(&self) -> bool {
        self.opacity < 1.0
            || self.blend != BlendMode::NORMAL
            || !self.filters.is_empty()
            || self.isolate
    }
}
