//! Filters applied to what is drawn beneath a rectangle.

use bytemuck::{Pod, Zeroable};

use crate::geometry::LocalRect;
use crate::item::{ClipRef, SpaceRef};

/// A filter chain applied to what is already drawn beneath a rectangle.
///
/// The backdrop reads the target it draws into over the read extent of its device bounding box,
/// applies the chain, and paints the result over `bounds`. The target is the innermost open group,
/// or the output when no group is open. The ink is the device bounding box of `bounds`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Backdrop {
    /// The rectangle the filtered content is painted over.
    pub bounds: LocalRect,
    /// The chain's index in the chunk's filter-chain list.
    pub filters: u32,
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
}
