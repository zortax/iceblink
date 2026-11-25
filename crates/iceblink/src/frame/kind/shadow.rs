//! Shadows with their ink.

use bytemuck::{Pod, Zeroable};

use crate::geometry::LocalRect;
use crate::item::Shadow;

/// A shadow with the rectangle it paints within.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct FrameShadow {
    /// The shadow's ink, in local space.
    pub bounds: LocalRect,
    /// The shadow, with frame slots.
    pub shadow: Shadow,
}
