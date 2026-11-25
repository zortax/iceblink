//! One instance of a vector-pass composite.

use bytemuck::{Pod, Zeroable};
use iceblink::geom::{Device, Rect};

/// One instance of a vector-pass composite.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct VectorInstance {
    pub(crate) bounds: [f32; 4],
    pub(crate) source: [f32; 4],
    pub(crate) control: [f32; 4],
}

impl VectorInstance {
    /// Returns the instance that composites `bounds` from the scratch texel `source` on, clipped
    /// by the frame clip slot `clip`.
    pub(crate) fn new(bounds: Rect<i32, Device>, source: (i32, i32), clip: u32) -> Self {
        Self {
            bounds: [
                bounds.origin.x as f32,
                bounds.origin.y as f32,
                bounds.size.width as f32,
                bounds.size.height as f32,
            ],
            source: [source.0 as f32, source.1 as f32, 0.0, 0.0],
            control: [f32::from_bits(clip), 0.0, 0.0, 0.0],
        }
    }
}
