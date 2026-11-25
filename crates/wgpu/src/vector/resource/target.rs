//! Where a rasteriser put a pass.

/// The scratch target a rasteriser put a pass into, which only that rasteriser resolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VectorTarget(pub u64);

impl VectorTarget {
    /// The target of a pass the rasteriser could not place.
    ///
    /// Such a pass was not rasterised, and the rasteriser reports the frame's preparation as
    /// `VectorError::OutOfCapacity`.
    pub const NONE: Self = Self(u64::MAX);
}
