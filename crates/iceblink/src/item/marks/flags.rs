//! How the prims of a marks item are mapped, combined and capped.

use bytemuck::{Pod, Zeroable};

/// How the prims of a marks item are mapped, combined and capped.
///
/// It is a bit set with the caps of polyline runs in two 2-bit fields.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct MarkFlags(pub u32);

impl MarkFlags {
    /// A single paint of the union of the prims' coverage.
    pub const UNION: Self = Self(1 << 0);

    /// Mapping of positions only by the axes, with lengths in local units.
    pub const SCREEN: Self = Self(1 << 1);

    /// Discs drawn as squares.
    pub const SQUARE_DISCS: Self = Self(1 << 2);

    /// The position of the 2-bit cap of every run start.
    pub const START_CAP_SHIFT: u32 = 8;

    /// The position of the 2-bit cap of every run end.
    pub const END_CAP_SHIFT: u32 = 10;

    /// A cap that stops at the end of the segment.
    pub const CAP_BUTT: u32 = 0;

    /// A cap that continues the segment by half the stroke width.
    pub const CAP_SQUARE: u32 = 1;

    /// A half disc of half the stroke width.
    pub const CAP_ROUND: u32 = 2;

    const BITS: u32 = Self::UNION.0 | Self::SCREEN.0 | Self::SQUARE_DISCS.0;

    /// Returns the cap bits for runs that start with `start` and end with `end`.
    pub const fn caps(start: u32, end: u32) -> Self {
        Self(((start & 3) << Self::START_CAP_SHIFT) | ((end & 3) << Self::END_CAP_SHIFT))
    }

    /// Returns the cap of every run start.
    pub const fn start_cap(self) -> u32 {
        (self.0 >> Self::START_CAP_SHIFT) & 3
    }

    /// Returns the cap of every run end.
    pub const fn end_cap(self) -> u32 {
        (self.0 >> Self::END_CAP_SHIFT) & 3
    }

    /// Reports whether every bit of `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Reports whether only listed bits are set and both caps are listed caps.
    pub const fn is_valid(self) -> bool {
        let caps = (3 << Self::START_CAP_SHIFT) | (3 << Self::END_CAP_SHIFT);
        self.0 & !(Self::BITS | caps) == 0
            && self.start_cap() <= Self::CAP_ROUND
            && self.end_cap() <= Self::CAP_ROUND
    }
}

impl core::ops::BitOr for MarkFlags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl core::ops::BitOrAssign for MarkFlags {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}
