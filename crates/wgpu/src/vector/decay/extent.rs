//! The size of a scratch texture.

/// The width, height and layer count of a scratch texture, in texels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extent {
    /// The width in texels.
    pub width: u32,
    /// The height in texels.
    pub height: u32,
    /// The number of layers.
    pub layers: u32,
}

impl Extent {
    /// The extent of nothing allocated.
    pub const NONE: Self = Self {
        width: 0,
        height: 0,
        layers: 0,
    };

    /// The granularity that [`classed`](Self::classed) rounds the width and height to.
    // A drag that grows the regions a few pixels per frame then reallocates once per 256 pixels
    // crossed, instead of on every step.
    pub const SIZE_CLASS: u32 = 256;

    /// Returns an extent of `width` by `height` in `layers` layers.
    pub fn new(width: u32, height: u32, layers: u32) -> Self {
        Self {
            width,
            height,
            layers,
        }
    }

    /// Returns this extent with the width and height rounded up to a multiple of
    /// [`SIZE_CLASS`](Self::SIZE_CLASS), and at least one class.
    ///
    /// The layers are kept.
    #[must_use]
    pub fn classed(self) -> Self {
        let up = |texels: u32| texels.max(1).div_ceil(Self::SIZE_CLASS) * Self::SIZE_CLASS;
        Self {
            width: up(self.width),
            height: up(self.height),
            layers: self.layers,
        }
    }

    /// Returns the smallest extent that contains both.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            width: self.width.max(other.width),
            height: self.height.max(other.height),
            layers: self.layers.max(other.layers),
        }
    }

    /// Reports whether every dimension is at least the one of `other`.
    pub fn covers(self, other: Self) -> bool {
        self.width >= other.width && self.height >= other.height && self.layers >= other.layers
    }
}
