//! How world matrices and clip chains follow scroll offsets.

use crate::geometry::bounds::DeviceBox;
use crate::geometry::coverage::Partial;

/// The scroll offsets a world matrix follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Follows {
    /// The matrix depends on no scroll offset.
    #[default]
    Still,
    /// The matrix maps the plane affinely and depends on the offset of the scroll node at this
    /// slot only, which moves it by a translation.
    Scroll(u32),
    /// The matrix depends on scroll offsets in any other way: on several, through a sticky
    /// or thumb anchor, or under a projective matrix.
    Mixed,
}

impl Follows {
    /// Returns what a matrix built from two parts follows, one following `self` and the other
    /// `other`.
    pub(crate) fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::Still, other) | (other, Self::Still) => other,
            (Self::Scroll(a), Self::Scroll(b)) if a == b => self,
            _ => Self::Mixed,
        }
    }
}

/// The link of a clip chain that decides its still bounds: the deepest link whose space follows
/// no scroll offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StillRoot {
    /// No still link in the chain.
    None,
    /// The link of the clip node at this slot.
    Node(u32),
    /// A chunk-local clip.
    Local,
}

/// The device bounds of a clip chain, split by whether each link's space follows a scroll
/// offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ClipSplit {
    /// What the spaces of the links that follow an offset follow, or `Still` when none does.
    pub(crate) follows: Follows,
    /// The bounds of the links whose space follows a scroll offset.
    pub(crate) moving: DeviceBox,
    /// The bounds of the links whose space follows none.
    pub(crate) still: DeviceBox,
    /// Where the links whose space follows none cover in part, or `None` when that cannot be
    /// bounded.
    pub(crate) partial: Option<Partial>,
    /// The deepest link whose space follows no offset. Its chain has the still bounds and
    /// partial cover of this chain.
    pub(crate) root: StillRoot,
}

impl ClipSplit {
    /// The split of a chain without links.
    pub(crate) const OPEN: Self = Self {
        follows: Follows::Still,
        moving: DeviceBox::EVERYWHERE,
        still: DeviceBox::EVERYWHERE,
        partial: Some(Partial::NONE),
        root: StillRoot::None,
    };

    /// Returns the split with one more link `link`, whose space follows `follows`, whose bounds
    /// are `bounds` and which covers in part where `partial` says.
    #[inline]
    pub(crate) fn with(
        self,
        link: StillRoot,
        follows: Follows,
        bounds: DeviceBox,
        partial: Option<Partial>,
    ) -> Self {
        match follows {
            Follows::Still => Self {
                still: self.still.intersect(bounds),
                partial: self
                    .partial
                    .zip(partial)
                    .map(|(held, added)| held.and(added)),
                root: link,
                ..self
            },
            Follows::Scroll(_) | Follows::Mixed => Self {
                follows: self.follows.and(follows),
                moving: self.moving.intersect(bounds),
                ..self
            },
        }
    }
}
