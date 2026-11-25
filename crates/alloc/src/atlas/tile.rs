//! Allocated tiles.

use iceblink::geom::{Device, Rect};

use crate::atlas::texture::TextureId;

/// The allocator's identity of one tile within its texture.
///
/// It is opaque and does not encode a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId(pub u32);

/// The texture and rectangle one cached entry occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasTile {
    /// The texture that holds the content.
    pub texture: TextureId,
    /// The allocation within the texture.
    pub tile: TileId,
    /// The rectangle of the content, in texels.
    pub bounds: Rect<i32, Device>,
}
