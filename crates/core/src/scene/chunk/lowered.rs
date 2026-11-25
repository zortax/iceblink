//! A chunk lowered into frame kinds, and what its atlas content depends on.

use std::sync::Arc;

use iceblink::{FramePath, LoweredChunk, Matrix4, SubpixelOrder};
use smallvec::SmallVec;

use crate::content::TileKey;

/// One lowering of a chunk, which a resident renderer keeps as one revision.
#[derive(Debug)]
pub(crate) struct Lowered {
    pub(crate) revision: u64,
    /// The inputs the lowering depended on beside the chunk itself.
    pub(crate) key: LoweringKey,
    /// The chunk offset the instances were lowered at.
    pub(crate) base: [f32; 2],
    pub(crate) chunk: Arc<LoweredChunk>,
    /// Where each item of the chunk went, by its position in the chunk.
    pub(crate) items: Arc<[LoweredItem]>,
    /// The pieces of items that lowered to more than one kind, in paint order.
    pub(crate) parts: Arc<[Part]>,
    /// The frame paths of the lowering, which are drawn transient.
    pub(crate) paths: Arc<[LoweredPath]>,
    /// What the atlas content of the lowering depends on, for a chunk with such content.
    pub(crate) content: Option<ContentStamp>,
    /// The combined clip chains the lowering draws through, which it holds until it is retired.
    pub(crate) combined: Box<[u32]>,
}

/// The inputs of a lowering that are not part of the chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LoweringKey {
    pub(crate) custom_shaders: bool,
    /// The frame clip slot every item is drawn through in addition to its own chain.
    pub(crate) context: u32,
    /// The stripe order glyph runs may use, for a chunk with glyph runs.
    pub(crate) subpixel: Option<SubpixelOrder>,
    /// Whether an isolating group draws the chunk, for a chunk with glyph runs.
    pub(crate) isolated: bool,
    /// The output scale as `f32` bits, for a chunk with glyph runs.
    pub(crate) scale_bits: u32,
}

/// What the atlas content of a lowering was made for.
#[derive(Debug)]
pub(crate) struct ContentStamp {
    /// The content generation the tiles belong to.
    pub(crate) epoch: u64,
    /// The state the content was last found to hold in.
    pub(crate) checked: Checked,
    /// The tiles the instances read.
    pub(crate) tiles: Arc<[TileKey]>,
    /// Whether the tiles hold the pin of the renderers that keep the lowering resident, which
    /// lasts until the lowering is retired or no renderer holds it.
    pub(crate) resident: bool,
    /// Each client space the content was placed under.
    pub(crate) worlds: SmallVec<[Placement; 1]>,
    /// Whether a frame path of the lowering takes masks when the chunk is lowered again.
    pub(crate) rerouted: bool,
}

/// A client space that a lowering placed sprites or frame paths under.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    pub(crate) slot: u32,
    /// The space's world matrix when the content was lowered.
    pub(crate) world: Matrix4,
    /// Whether the space was moving when the content was lowered.
    pub(crate) moving: bool,
    /// Whether sprites were placed under the space, which hold only on the same pixel phase.
    /// Frame paths hold under any translation.
    pub(crate) phased: bool,
}

/// A frame path of a lowering.
#[derive(Clone, Debug)]
pub(crate) struct LoweredPath {
    /// The path as lowered under its space's world matrix at the lowering's base offset.
    pub(crate) path: FramePath,
    /// The frame clip slot the path is drawn through.
    pub(crate) clip: u32,
    /// The client space slot the path was placed under.
    pub(crate) space: u32,
}

/// The counts and the chunk offset under which a lowering's atlas content last held.
///
/// While they are unchanged, the content still holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Checked {
    /// The content's placement count.
    pub(crate) placements: u64,
    /// The evictions the content cache had made.
    pub(crate) evictions: u64,
    pub(crate) offset: [f32; 2],
}

/// Where one item of a chunk went when the chunk was lowered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoweredItem {
    /// The item lowered to nothing.
    Nothing,
    /// The item lowered to instances `start..start + len` of the lowered chunk's array for
    /// `lane`.
    Instances { lane: Lane, start: u32, len: u32 },
    /// The item lowered to parts `start..start + len` of the lowering.
    Parts { start: u32, len: u32 },
    /// The item is drawn on its own, as a path, an external texture or a backdrop.
    Single,
}

/// A piece of an item that lowered to more than one kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// Instances `start..start + len` of the lowered chunk's array for `lane`.
    Instances { lane: Lane, start: u32, len: u32 },
    /// A path of the lowering.
    Path(u32),
}

/// An instanced frame kind, by the lane that carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Lane {
    Quads,
    ShaderQuads,
    Shadows,
    Lines,
    Mono,
    Subpixel,
    Color,
    Marks,
}

impl Lane {
    pub(crate) const COUNT: usize = 8;

    /// Every lane, in the order of their discriminants.
    pub(crate) const ALL: [Lane; Lane::COUNT] = [
        Lane::Quads,
        Lane::ShaderQuads,
        Lane::Shadows,
        Lane::Lines,
        Lane::Mono,
        Lane::Subpixel,
        Lane::Color,
        Lane::Marks,
    ];

    /// Reports whether the lane carries sprites, whose ink is their own when an item has several.
    pub(crate) fn sprite(self) -> bool {
        matches!(self, Lane::Mono | Lane::Subpixel | Lane::Color)
    }

    /// Returns how many instances a lowered chunk holds in this lane.
    pub(crate) fn len(self, chunk: &LoweredChunk) -> usize {
        match self {
            Lane::Quads => chunk.quads.len(),
            Lane::ShaderQuads => chunk.shader_quads.len(),
            Lane::Shadows => chunk.shadows.len(),
            Lane::Lines => chunk.lines.len(),
            Lane::Mono => chunk.mono.len(),
            Lane::Subpixel => chunk.subpixel.len(),
            Lane::Color => chunk.color.len(),
            Lane::Marks => chunk.marks.len(),
        }
    }
}
