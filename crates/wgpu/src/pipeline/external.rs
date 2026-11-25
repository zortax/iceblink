//! The block an external quad draws with.

use bytemuck::{Pod, Zeroable};
use iceblink::ExternalQuad;

/// The uniform block of an external quad.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct ExternalParams {
    pub(crate) bounds: [f32; 4],
    pub(crate) control: [f32; 4],
}

impl ExternalParams {
    /// Returns the block of `quad`, whose texture is `premultiplied` or straight.
    pub(crate) fn of(quad: &ExternalQuad, premultiplied: bool) -> Self {
        Self {
            bounds: quad.bounds,
            control: [
                f32::from_bits(quad.clip.0),
                f32::from_bits(quad.space.0),
                quad.opacity.clamp(0.0, 1.0),
                f32::from(premultiplied),
            ],
        }
    }
}
