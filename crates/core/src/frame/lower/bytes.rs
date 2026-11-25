//! The memory a chunk's recording and lowerings occupy.

use std::sync::Arc;

use iceblink::{LoweredChunk, RecordedChunk};

use crate::scene::chunk::ChunkRecord;

/// Returns the bytes a chunk's recording and lowerings occupy, including the glyph runs kept
/// lowered for its next lowering.
pub(crate) fn lowered_bytes(record: &ChunkRecord) -> u64 {
    let recorded = recorded_bytes(&record.recorded);
    let lowered = record
        .derived
        .lowered
        .as_ref()
        .map_or(0, |lowered| chunk_bytes(&lowered.chunk));
    let runs: usize = record
        .derived
        .lowered_runs
        .iter()
        .flatten()
        .map(|run| {
            core::mem::size_of_val(run.lowering.items.as_slice())
                + core::mem::size_of_val(run.lowering.tiles.as_slice())
        })
        .sum();
    recorded + lowered + runs as u64
}

fn chunk_bytes(chunk: &Arc<LoweredChunk>) -> u64 {
    (core::mem::size_of_val(chunk.quads.as_slice())
        + core::mem::size_of_val(chunk.shader_quads.as_slice())
        + core::mem::size_of_val(chunk.shadows.as_slice())
        + core::mem::size_of_val(chunk.lines.as_slice())
        + core::mem::size_of_val(chunk.mono.as_slice())
        + core::mem::size_of_val(chunk.subpixel.as_slice())
        + core::mem::size_of_val(chunk.color.as_slice())
        + core::mem::size_of_val(chunk.marks.as_slice())) as u64
}

/// Returns the item and arena bytes of a recorded chunk.
pub(crate) fn recorded_bytes(chunk: &RecordedChunk) -> u64 {
    (core::mem::size_of_val(chunk.ops.as_slice())
        + core::mem::size_of_val(chunk.quads.as_slice())
        + core::mem::size_of_val(chunk.shader_quads.as_slice())
        + core::mem::size_of_val(chunk.shadows.as_slice())
        + core::mem::size_of_val(chunk.lines.as_slice())
        + core::mem::size_of_val(chunk.glyph_runs.as_slice())
        + core::mem::size_of_val(chunk.images.as_slice())
        + core::mem::size_of_val(chunk.marks.as_slice())
        + core::mem::size_of_val(chunk.paths.as_slice())
        + core::mem::size_of_val(chunk.externals.as_slice())
        + core::mem::size_of_val(chunk.backdrops.as_slice())
        + core::mem::size_of_val(chunk.glyphs.as_slice())
        + core::mem::size_of_val(chunk.coords.as_slice())) as u64
}
