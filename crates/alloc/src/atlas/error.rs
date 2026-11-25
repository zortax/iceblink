//! Atlas errors.

use iceblink::geom::{Device, Size};

use crate::atlas::sink::SinkError;

/// An error from an atlas operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AtlasError {
    /// The tile is larger than the largest texture the atlas may create.
    #[error(
        "a {}x{} tile does not fit a texture capped at {}x{}",
        requested.width,
        requested.height,
        limit.width,
        limit.height
    )]
    TooLarge {
        /// The requested tile size.
        requested: Size<i32, Device>,
        /// The largest texture size.
        limit: Size<i32, Device>,
    },
    /// The tile size is not positive, or the pool has no room for the tile and may not grow.
    ///
    /// For a positive size, evicting and retrying can succeed.
    #[error("the pool has no room for a {}x{} tile", requested.width, requested.height)]
    OutOfSpace {
        /// The requested tile size.
        requested: Size<i32, Device>,
    },
    /// The byte count does not match the tile size and format.
    #[error("a {}x{} tile of this format needs {expected} bytes, not {actual}", size.width, size.height)]
    WrongByteCount {
        /// The tile size.
        size: Size<i32, Device>,
        /// The byte count the size and format require.
        expected: u64,
        /// The byte count supplied.
        actual: u64,
    },
    /// The sink refused an operation.
    #[error(transparent)]
    Sink(#[from] SinkError),
}
