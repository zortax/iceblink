//! Instances of the instanced frame kinds.

use iceblink::{DeviceRect, ItemSource};

use crate::plan::order::DrawOrder;

/// An instanced frame kind, one per lane of [`Lanes`](iceblink::Lanes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LaneKind {
    /// `Lanes::shadows`.
    Shadows,
    /// `Lanes::quads`.
    Quads,
    /// `Lanes::shader_quads`.
    ShaderQuads,
    /// `Lanes::marks`.
    Marks,
    /// `Lanes::lines`.
    Lines,
    /// `Lanes::mono`.
    Mono,
    /// `Lanes::subpixel`.
    Subpixel,
    /// `Lanes::color`.
    Color,
}

impl LaneKind {
    /// Every lane kind, in the order the planner keeps its lanes.
    pub const ALL: [Self; 8] = [
        Self::Shadows,
        Self::Quads,
        Self::ShaderQuads,
        Self::Marks,
        Self::Lines,
        Self::Mono,
        Self::Subpixel,
        Self::Color,
    ];
}

/// One instance of an instanced frame kind, as the planner reads it.
///
/// The source locates the instance's data, in a resident chunk or in the lane's `items`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instance {
    /// The lane the instance is drawn in.
    pub lane: LaneKind,
    /// The run written into the lane's `sources`, which the planner joins with the run before it
    /// when this one continues it.
    pub source: ItemSource,
    /// The device ink: the pixels the instance can change, cut to its clip.
    pub ink: DeviceRect,
    /// The binding of the instance, which sorts instances of equal order within the lane.
    ///
    /// The batches of each lane split on it as follows:
    ///
    /// - For a sprite lane, `key[0]` is the atlas page, and a batch holds one page. `key[1]`
    ///   only sorts.
    /// - For the shader quad lane, `key` is the frame shader slot and the parameter slot, and a
    ///   batch holds one pair.
    /// - For every other lane, the key only sorts.
    pub key: [u32; 2],
    /// The instance's order among the instances of its block, counting from one, or zero outside
    /// a block.
    pub local: DrawOrder,
    /// Whether the instance is a mark with `MarkFlags::UNION`, which gets a bin in the mark plan.
    pub union: bool,
}

impl Instance {
    /// Returns an instance of `lane` with a zero key, outside any block.
    pub const fn new(lane: LaneKind, source: ItemSource, ink: DeviceRect) -> Self {
        Self {
            lane,
            source,
            ink,
            key: [0, 0],
            local: 0,
            union: false,
        }
    }

    /// Returns the instance with `key`.
    pub const fn keyed(mut self, key: [u32; 2]) -> Self {
        self.key = key;
        self
    }

    /// Returns the instance with the local order `local` of its block.
    pub const fn at_local(mut self, local: DrawOrder) -> Self {
        self.local = local;
        self
    }

    /// Returns the instance marked as a union mark.
    pub const fn united(mut self) -> Self {
        self.union = true;
        self
    }
}
