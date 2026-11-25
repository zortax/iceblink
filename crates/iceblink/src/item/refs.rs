//! The transform node and the clip chain an item is drawn under.

use bytemuck::{Pod, Zeroable};

/// A transform node an item is drawn under, as the node's slot.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct SpaceRef(pub u32);

impl SpaceRef {
    /// The chunk's default space, `ChunkProps::space`.
    pub const INHERIT: Self = Self(u32::MAX);
}

/// A clip chain an item is drawn through.
///
/// A value is decoded in this order:
///
/// 1. [`ClipRef::INHERIT`] is the chunk's default clip.
/// 2. [`ClipRef::NONE`] is no clip at all.
/// 3. A value with the [`ClipRef::LOCAL`] bit is a chunk-local clip, and the bits below it are its
///    index in the chunk.
/// 4. Any other value is the slot of a clip node, whose whole chain applies.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct ClipRef(pub u32);

impl ClipRef {
    /// The chunk's default clip, `ChunkProps::clip`.
    pub const INHERIT: Self = Self(u32::MAX);

    /// No clip at all.
    pub const NONE: Self = Self(u32::MAX - 1);

    /// The bit that marks a chunk-local clip.
    pub const LOCAL: u32 = 1 << 31;

    /// Returns the reference to the chunk-local clip at `index`.
    ///
    /// Only the low 24 bits of `index` are kept.
    pub const fn local(index: u32) -> Self {
        Self(Self::LOCAL | (index & 0x00ff_ffff))
    }

    /// Returns the index of the chunk-local clip this value refers to, if it refers to one.
    pub const fn local_index(self) -> Option<u32> {
        if self.0 == Self::INHERIT.0 || self.0 == Self::NONE.0 || self.0 & Self::LOCAL == 0 {
            None
        } else {
            Some(self.0 & !Self::LOCAL)
        }
    }
}
