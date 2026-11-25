//! Appending to the arenas and lists of a chunk.

use std::sync::Arc;

use crate::chunk::LocalClip;
use crate::item::{ArenaRange, ClipRef, Glyph, MarkPayload};
use crate::node::Filter;
use crate::transaction::recorder::{ChunkRecorder, index_of_push};

/// The most chunk-local clips a chunk holds.
const LOCAL_CLIP_LIMIT: usize = 1 << 24;

impl ChunkRecorder<'_> {
    /// Appends glyphs to the chunk's glyph arena and returns their range.
    pub fn glyphs(&mut self, glyphs: &[Glyph]) -> ArenaRange {
        extend(&mut self.chunk.glyphs, glyphs)
    }

    /// Appends variation coordinates to the chunk's coordinate arena and returns their range.
    pub fn coords(&mut self, coords: &[i16]) -> ArenaRange {
        extend(&mut self.chunk.coords, coords)
    }

    /// Adds a payload to the chunk's payload list and returns its index.
    pub fn payload(&mut self, payload: Arc<MarkPayload>) -> u32 {
        index_of_push(&mut self.chunk.payloads, payload)
    }

    /// Adds a filter chain to the chunk's filter-chain list and returns its index.
    pub fn filters(&mut self, chain: Arc<[Filter]>) -> u32 {
        index_of_push(&mut self.chunk.filters, chain)
    }

    /// Adds a chunk-local clip and returns the reference items use to be drawn through it.
    ///
    /// # Panics
    ///
    /// Panics if the chunk already holds 2²⁴ local clips.
    pub fn local_clip(&mut self, clip: LocalClip) -> ClipRef {
        assert!(
            self.chunk.local_clips.len() < LOCAL_CLIP_LIMIT,
            "a chunk holds at most 2^24 local clips"
        );
        ClipRef::local(index_of_push(&mut self.chunk.local_clips, clip))
    }
}

/// Appends `values` to `arena` and returns their range.
fn extend<T: Copy>(arena: &mut Vec<T>, values: &[T]) -> ArenaRange {
    let start = arena.len() as u32;
    arena.extend_from_slice(values);
    ArenaRange {
        start,
        len: values.len() as u32,
    }
}
