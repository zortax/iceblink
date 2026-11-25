//! The instanced frame kinds, one lane per kind.

use bytemuck::{Pod, Zeroable};

use crate::frame::{ColorSprite, FrameMarkPayload, FrameMarks, FrameShadow, Sprite};
use crate::item::{Line, Quad, ShaderQuad};

/// The frame clip slot of an instance drawn without a clip.
pub const FRAME_NO_CLIP: u32 = u32::MAX;

/// The instanced frame kinds, one lane per kind.
#[derive(Debug, Default)]
pub struct Lanes {
    /// Quads.
    pub quads: Lane<Quad>,
    /// Shader quads.
    pub shader_quads: Lane<ShaderQuad>,
    /// Shadows.
    pub shadows: Lane<FrameShadow>,
    /// Lines.
    pub lines: Lane<Line>,
    /// Glyph sprites and path masks with one coverage per texel.
    pub mono: Lane<Sprite>,
    /// Glyph sprites with one coverage per colour stripe.
    pub subpixel: Lane<Sprite>,
    /// Colour glyphs and images.
    pub color: Lane<ColorSprite>,
    /// Marks.
    pub marks: Lane<FrameMarks>,
    /// The payloads of transient marks, indexed by `Marks::payload`.
    pub mark_payloads: Vec<FrameMarkPayload>,
}

impl Lanes {
    /// Empties every lane, keeping the allocations.
    pub fn clear(&mut self) {
        self.quads.clear();
        self.shader_quads.clear();
        self.shadows.clear();
        self.lines.clear();
        self.mono.clear();
        self.subpixel.clear();
        self.color.clear();
        self.marks.clear();
        self.mark_payloads.clear();
    }
}

/// The instances of one frame kind, in draw order.
///
/// `sources` lists every instance drawn, as runs of consecutive instances. A resident run holds
/// instances of a resident chunk, and a transient run holds instances of `items`.
#[derive(Debug)]
pub struct Lane<T> {
    /// The instances whose only copy is this frame, at their final positions.
    pub items: Vec<T>,
    /// Every instance drawn, in draw order, as runs.
    pub sources: Vec<ItemSource>,
}

impl<T> Lane<T> {
    /// Empties the lane, keeping the allocations.
    pub fn clear(&mut self) {
        self.items.clear();
        self.sources.clear();
    }

    /// Returns how many instances the lane draws.
    pub fn drawn(&self) -> usize {
        self.sources.iter().map(|run| run.len as usize).sum()
    }

    /// Returns every instance drawn, in draw order, each as a run of one.
    ///
    /// The position of an instance in this sequence is its position in the lane, which batch
    /// ranges, opaque covers and mark bins count in.
    pub fn instances(&self) -> impl Iterator<Item = ItemSource> + '_ {
        self.sources.iter().flat_map(|run| {
            run.indices().map(move |index| ItemSource {
                chunk: run.chunk,
                index,
                len: 1,
            })
        })
    }
}

impl<T> Default for Lane<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            sources: Vec::new(),
        }
    }
}

/// A run of drawn instances, drawn in array order.
///
/// A resident run is the `len` instances from `index` of `drawn[chunk]`'s lowered chunk, in the
/// array of the lane's kind, moved by that drawn chunk's offset. A transient run is
/// `items[index..index + len]` of the lane.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct ItemSource {
    /// An index into `Frame::drawn`, or [`ItemSource::TRANSIENT`].
    pub chunk: u32,
    /// The first instance, an index into the resident chunk's array for the lane's kind or into
    /// `Lane::items`.
    pub index: u32,
    /// How many instances the run draws, at least one.
    pub len: u32,
}

impl ItemSource {
    /// The `chunk` of a transient run.
    pub const TRANSIENT: u32 = u32::MAX;

    /// Returns the run of the one instance `items[index]`.
    pub const fn transient(index: u32) -> Self {
        Self {
            chunk: Self::TRANSIENT,
            index,
            len: 1,
        }
    }

    /// Returns the run of the one instance `index` of `drawn[chunk]`.
    pub const fn resident(chunk: u32, index: u32) -> Self {
        Self {
            chunk,
            index,
            len: 1,
        }
    }

    /// Reports whether the run draws instances of `Lane::items`.
    pub const fn is_transient(self) -> bool {
        self.chunk == Self::TRANSIENT
    }

    /// Returns the indices of the instances the run draws.
    pub const fn indices(self) -> core::ops::Range<u32> {
        self.index..self.index.saturating_add(self.len)
    }
}
