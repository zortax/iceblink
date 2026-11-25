//! Shared payloads of shapes drawn as one item.

mod flags;
mod payload;

use bytemuck::{Pod, Zeroable};

use crate::geometry::LocalRect;
use crate::item::{ClipRef, SpaceRef};
use crate::paint::PaintRef;

pub use self::flags::MarkFlags;
pub use self::payload::{MarkBox, MarkPayload, MarkPlacement};

/// One shared payload of shapes drawn as one item, such as chart points, series lines or
/// repeated icons.
///
/// The item draws `discs` discs, `boxes` boxes, the polyline segments among `vertices` vertices
/// and `placements` placements, each counted from the start of its payload list. When exactly one
/// count is nonzero, `first` offsets that list, and otherwise `first` is zero. Counts that reach
/// past the payload are invalid.
///
/// Every prim is filled with `paint`. Nothing is drawn outside `bounds`, and the ink is `bounds`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Marks {
    /// The rectangle drawing is cut to, which contains every prim.
    pub bounds: LocalRect,
    /// The paint of every prim.
    pub paint: PaintRef,
    /// The offset subtracted from the local point before the paint is evaluated.
    pub paint_origin: [f32; 2],
    /// The offset added to every mapped payload position.
    pub origin: [f32; 2],
    /// The linear map from payload to local space, `[a, b, c, d]`: `x' = a x + c y` and
    /// `y' = b x + d y`.
    pub axes: [f32; 4],
    /// Half the width of the polyline stroke.
    pub half_width: f32,
    /// The number of discs drawn.
    pub discs: u32,
    /// The number of boxes drawn.
    pub boxes: u32,
    /// The number of polyline vertices read, separators included.
    pub vertices: u32,
    /// The number of placements drawn.
    pub placements: u32,
    /// The first prim drawn, for an item that draws a single payload kind.
    pub first: u32,
    /// The payload's index in the chunk's payload list.
    pub payload: u32,
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
    /// How the prims are mapped, combined and capped.
    pub flags: MarkFlags,
}
