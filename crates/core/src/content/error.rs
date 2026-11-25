//! Why content could not be lowered.

use core::fmt;

use iceblink_alloc::atlas::AtlasError;

/// The reason an item drew nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContentError {
    /// The item refers to an image the image store does not hold.
    UnknownImage,
    /// The item refers to an image with no texels.
    EmptyImage,
    /// The atlas refused a tile, even after evicting unpinned tiles.
    Atlas(AtlasError),
}

impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownImage => formatter.write_str("the item refers to an unknown image"),
            Self::EmptyImage => formatter.write_str("the item refers to an image with no texels"),
            Self::Atlas(error) => write!(formatter, "the atlas refused a tile: {error}"),
        }
    }
}

impl std::error::Error for ContentError {}

impl From<AtlasError> for ContentError {
    fn from(error: AtlasError) -> Self {
        Self::Atlas(error)
    }
}
