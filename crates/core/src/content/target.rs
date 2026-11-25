//! Where a lowered item is drawn: its device transform, its chunk offset and its frame slots.

use iceblink::geom::{Affine2, Matrix4};

/// Where one item is drawn.
///
/// The item's own `clip` and `space` fields are ignored. Its paint references must already be
/// frame paint slots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    /// The world matrix of the item's space, from local space to device space.
    pub world: Matrix4,
    /// The chunk offset, in local units, added to the item's geometry.
    pub offset: [f32; 2],
    /// The frame clip slot, or `FRAME_NO_CLIP`.
    pub clip: u32,
    /// The frame space slot.
    pub space: u32,
    /// Whether the world matrix is moving, which lets glyph tiles stretch under a slight scale.
    pub moving: bool,
}

impl Target {
    /// Returns a target under `world` with no offset, no clip and space slot zero.
    pub fn new(world: Matrix4) -> Self {
        Self {
            world,
            offset: [0.0; 2],
            clip: iceblink::FRAME_NO_CLIP,
            space: 0,
            moving: false,
        }
    }

    /// Returns the world matrix as a two-dimensional transform, if it is one.
    pub(crate) fn affine(&self) -> Option<Affine2> {
        self.world.to_affine2()
    }
}

/// Reports whether sprites lowered under `before` stay on whole device pixels at the same phase
/// under `after`.
///
/// This holds when both matrices are two-dimensional with the same linear part, and their
/// translations differ by whole device pixels. Glyph runs and path masks lowered under `before`
/// are lowered again when it does not hold. Images and marks keep their pixels under any move.
pub fn keeps_pixels(before: &Matrix4, after: &Matrix4) -> bool {
    let (Some(before), Some(after)) = (before.to_affine2(), after.to_affine2()) else {
        return false;
    };

    let linear = |affine: Affine2| [affine.a, affine.b, affine.c, affine.d];
    let whole = |delta: f32| delta.fract() == 0.0;
    linear(before) == linear(after) && whole(after.tx - before.tx) && whole(after.ty - before.ty)
}
