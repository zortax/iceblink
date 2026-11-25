//! Why a rasteriser failed.

use core::fmt;

/// A failed rasterisation.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VectorError {
    /// The work exceeded a fixed capacity of the rasteriser.
    OutOfCapacity {
        /// What ran out.
        detail: String,
        /// How many passes, counted from the first, were rasterised before it ran out.
        ///
        /// Those passes are rasterised and can be composited.
        prepared: usize,
    },
    /// A resource could not be created.
    Allocation {
        /// What could not be allocated.
        detail: String,
    },
    /// The device rejected the work or was lost.
    Device {
        /// What the device reported.
        detail: String,
    },
}

impl fmt::Display for VectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfCapacity { detail, prepared } => write!(
                f,
                "the rasteriser ran out of capacity after {prepared} pass(es): {detail}"
            ),
            Self::Allocation { detail } => write!(f, "the rasteriser could not allocate: {detail}"),
            Self::Device { detail } => write!(f, "the device rejected the rasterisation: {detail}"),
        }
    }
}

impl std::error::Error for VectorError {}
