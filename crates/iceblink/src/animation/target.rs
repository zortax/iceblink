//! The properties an animation drives.

use crate::id::{EffectId, ScrollId, TransformId};

/// The property an animation drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationTarget {
    /// The local matrix of a transform node, with `Matrix` keyframes.
    Transform(TransformId),
    /// The opacity of an effect node, with `Scalar` keyframes.
    Opacity(EffectId),
    /// The offset of a scroll node, with `Vector` keyframes.
    ScrollOffset(ScrollId),
    /// A parameter of one filter in an effect's chain.
    Filter {
        /// The effect node whose chain holds the filter.
        effect: EffectId,
        /// The filter's position in the effect's chain.
        index: u8,
        /// The parameter animated.
        param: FilterParam,
    },
}

/// The parameter of a filter an animation drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterParam {
    /// The deviation of `Blur` or `DropShadow`, or the amount of `Opacity`, with `Scalar`
    /// keyframes.
    Amount,
    /// The offset of `DropShadow`, with `Vector` keyframes.
    Offset,
}
