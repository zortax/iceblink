//! The clip chains vector planning reads, as frame clip slots.

use iceblink::FRAME_NO_CLIP;

/// The frame clip table, as vector planning reads and extends it.
///
/// A chain is a frame clip slot, and [`FRAME_NO_CLIP`] is the empty chain. A chain's parent is the
/// same chain without its innermost link. The compositor implements this trait over the frame
/// clip table it builds.
pub trait ClipChains {
    /// Returns the parent of the chain `clip`, which is [`FRAME_NO_CLIP`] for a chain of one link.
    ///
    /// `clip` is never [`FRAME_NO_CLIP`].
    fn parent(&self, clip: u32) -> u32;

    /// Reports whether a link of the chain `clip` is a mask.
    fn has_mask(&self, clip: u32) -> bool;

    /// Returns the frame clip slot of the links of `clip` below its ancestor `ancestor`, as a
    /// chain of their own, outermost first.
    ///
    /// `ancestor` is a proper ancestor of `clip`, possibly the empty chain. The chain returned and
    /// `ancestor` together admit exactly what `clip` admits.
    fn residual(&mut self, clip: u32, ancestor: u32) -> u32;
}

/// Returns how many links the chain `clip` holds.
pub(crate) fn depth(clips: &impl ClipChains, clip: u32) -> u32 {
    let mut depth = 0;
    let mut cursor = clip;
    while cursor != FRAME_NO_CLIP {
        depth += 1;
        cursor = clips.parent(cursor);
    }
    depth
}

/// Returns the deepest chain that both `left` and `right` extend.
pub(crate) fn common_ancestor(clips: &impl ClipChains, left: u32, right: u32) -> u32 {
    let (mut left, mut right) = (left, right);
    let (mut left_depth, mut right_depth) = (depth(clips, left), depth(clips, right));
    while left_depth > right_depth {
        left = clips.parent(left);
        left_depth -= 1;
    }
    while right_depth > left_depth {
        right = clips.parent(right);
        right_depth -= 1;
    }
    while left != right {
        left = clips.parent(left);
        right = clips.parent(right);
    }
    left
}

/// Returns the residual of `clip` below `ancestor`, which is [`FRAME_NO_CLIP`] when they are the
/// same chain.
pub(crate) fn residual(clips: &mut impl ClipChains, clip: u32, ancestor: u32) -> u32 {
    if clip == ancestor {
        return FRAME_NO_CLIP;
    }
    clips.residual(clip, ancestor)
}
