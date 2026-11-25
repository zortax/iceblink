//! Textures the renderer did not draw.

use bytemuck::{Pod, Zeroable};

use crate::geometry::LocalRect;
use crate::item::{ClipRef, SpaceRef};

/// A quad that shows a texture the renderer did not draw, such as a video frame or a surface of
/// another process.
///
/// The texture is stretched over `bounds`. A renderer holding no texture for the slot draws
/// nothing. The ink is `bounds`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct ExternalQuad {
    /// The rectangle the texture covers.
    pub bounds: LocalRect,
    /// The external texture's slot.
    pub texture: u32,
    /// A multiplier on the result, clamped to `[0, 1]`.
    pub opacity: f32,
    /// The clip chain the item is clipped by.
    pub clip: ClipRef,
    /// The transform node whose space the item is in.
    pub space: SpaceRef,
}
