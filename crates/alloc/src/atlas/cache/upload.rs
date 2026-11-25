//! Queued uploads.

use std::sync::Arc;

use iceblink::geom::{Device, Rect};

use crate::atlas::texture::TextureId;
use crate::atlas::tile::TileId;

/// Texels queued for upload, owned by the queue or shared with their producer.
#[derive(Clone, Debug)]
pub enum UploadBytes {
    /// Texels owned by the queue.
    Owned(Vec<u8>),
    /// Texels shared with their producer, released after the write.
    Shared(Arc<Vec<u8>>),
}

impl UploadBytes {
    /// Returns the texels.
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared(bytes) => bytes,
        }
    }

    /// Returns the number of bytes.
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Reports whether there are no bytes.
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }
}

impl From<Vec<u8>> for UploadBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Owned(bytes)
    }
}

impl From<Arc<Vec<u8>>> for UploadBytes {
    fn from(bytes: Arc<Vec<u8>>) -> Self {
        Self::Shared(bytes)
    }
}

/// Bytes waiting to be written into a tile.
#[derive(Clone, Debug)]
pub(crate) struct PendingUpload {
    pub(crate) texture: TextureId,
    /// The tile the bytes belong to, so removing the tile discards them.
    pub(crate) tile: TileId,
    /// The level of detail, zero except for the levels of a standalone tile.
    pub(crate) mip: u32,
    /// The rectangle to write, in the coordinates of the level.
    pub(crate) bounds: Rect<i32, Device>,
    /// Tightly packed rows of texels.
    pub(crate) bytes: UploadBytes,
}
