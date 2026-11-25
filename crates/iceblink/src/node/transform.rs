//! Transform nodes.

use crate::geometry::{LocalRect, Matrix4};
use crate::id::{ScrollId, TransformId};

/// A node of the transform tree, whose local space items are recorded in.
///
/// A node's world matrix maps its local space to device space:
/// `world(n) = world(parent) · S(n) · local(n) · O(n)`. `O(n)` translates by the negated current
/// offset and overscroll displacement of `scroll`, when it is set. `S(n)` is the sticky offset of
/// a sticky anchor or the thumb offset of a thumb anchor. A node without a parent maps into
/// device space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformNode {
    /// The parent node, or `None` for a root.
    pub parent: Option<TransformId>,
    /// The map from this node's local space into its parent's.
    pub local: Matrix4,
    /// The scroll node whose offset this node applies.
    pub scroll: Option<ScrollId>,
    /// How the node follows the scroll offsets above it.
    pub anchor: Anchor,
}

/// How a transform node follows the scroll offsets of its ancestors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// Moves with every scroll offset above it.
    Scrolls,
    /// Takes none of the scroll offsets above it.
    Fixed,
    /// Moves with the scroll offsets above it, held inside a scroll port by insets.
    Sticky(StickyAnchor),
    /// Moves with the scroll offsets above it, and along a track by the offset of a scroll node.
    Thumb(ThumbAnchor),
}

/// The geometry of a sticky transform node.
///
/// The sticky offset is a translation in the parent's space, so the parent's space differs from
/// the scroll node's space by a translation only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickyAnchor {
    /// The scroll node whose port the node sticks to.
    pub scroll: ScrollId,
    /// The sticky rectangle, in the scroll node's space at zero offset.
    pub rect: LocalRect,
    /// The rectangle the sticky rectangle stays inside, in the same space.
    pub limit: LocalRect,
    /// Insets from the port's edges, as `[top, right, bottom, left]`. An edge set to NaN is free.
    pub inset: [f32; 4],
}

/// The geometry of a thumb transform node, such as a scrollbar thumb.
///
/// The thumb offset is a translation in the parent's space. On each axis it is the track's
/// length on that axis times the scroll node's current offset divided by its largest offset, so
/// the node sits at the start of its track at offset zero and at the end at the largest offset.
/// An axis whose largest offset is zero does not move. The overscroll displacement does not move
/// the node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThumbAnchor {
    /// The scroll node whose offset moves the node.
    pub scroll: ScrollId,
    /// How far the node moves over the scroll node's whole range, in the parent's units, as
    /// `[x, y]`.
    pub track: [f32; 2],
}
