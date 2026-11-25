//! Recording, moving and removing chunks.

use std::sync::Arc;

use iceblink::{
    Backdrop, ChunkKey, ChunkProps, ExternalQuad, Glyph, GlyphRun, Image, Line, Marks, Quad,
    RecordedChunk, RejectReason, ShaderQuad, Shadow,
};

use crate::naive::commit::check::{finite, nodes};
use crate::naive::commit::draft::Draft;
use crate::naive::scene::Chunk;

/// Stores a recorded chunk at offset zero.
///
/// The ids its props, local clips and filter chains refer to, and its props' flags, are checked
/// here.
/// The rest of the chunk is checked once the transaction is applied, against the state it
/// leaves.
pub(super) fn record(
    draft: &mut Draft<'_>,
    chunk: RecordedChunk,
    index: u32,
) -> Result<(), RejectReason> {
    nodes::props_ids(&draft.client, &chunk.props)?;
    for clip in &chunk.local_clips {
        nodes::local_clip_ids(&draft.client, clip)?;
    }
    for chain in &chunk.filters {
        nodes::filter_ids(&draft.client, chain)?;
    }
    nodes::props_values(&chunk.props)?;

    draft.counters.chunks_recorded += 1;
    draft.counters.bytes_recorded += bytes(&chunk);
    draft.recorded.insert(chunk.key, index);
    let stored = Chunk {
        props: chunk.props,
        recorded: Arc::new(chunk),
        offset: [0.0; 2],
    };
    draft.client.chunks.insert(stored.recorded.key, stored);
    Ok(())
}

pub(super) fn set_props(
    draft: &mut Draft<'_>,
    key: ChunkKey,
    props: ChunkProps,
) -> Result<(), RejectReason> {
    nodes::props_ids(&draft.client, &props)?;
    if !draft.client.chunks.contains_key(&key) {
        return Err(RejectReason::UnknownChunk);
    }

    nodes::props_values(&props)?;
    draft
        .client
        .chunks
        .get_mut(&key)
        .expect("checked above")
        .props = props;
    Ok(())
}

pub(super) fn set_offset(
    draft: &mut Draft<'_>,
    key: ChunkKey,
    offset: [f32; 2],
) -> Result<(), RejectReason> {
    let chunk = draft
        .client
        .chunks
        .get_mut(&key)
        .ok_or(RejectReason::UnknownChunk)?;
    finite::numbers(&offset)?;
    chunk.offset = offset;
    draft.counters.offsets_written += 1;
    Ok(())
}

pub(super) fn remove(draft: &mut Draft<'_>, key: ChunkKey) -> Result<(), RejectReason> {
    draft
        .client
        .chunks
        .remove(&key)
        .ok_or(RejectReason::UnknownChunk)?;
    draft.recorded.remove(&key);
    draft.counters.chunks_removed += 1;
    Ok(())
}

/// Returns the bytes of a chunk's items and arenas.
fn bytes(chunk: &RecordedChunk) -> u64 {
    let size = |count: usize, each: usize| (count * each) as u64;
    size(chunk.quads.len(), size_of::<Quad>())
        + size(chunk.shader_quads.len(), size_of::<ShaderQuad>())
        + size(chunk.shadows.len(), size_of::<Shadow>())
        + size(chunk.lines.len(), size_of::<Line>())
        + size(chunk.glyph_runs.len(), size_of::<GlyphRun>())
        + size(chunk.images.len(), size_of::<Image>())
        + size(chunk.marks.len(), size_of::<Marks>())
        + size(chunk.externals.len(), size_of::<ExternalQuad>())
        + size(chunk.backdrops.len(), size_of::<Backdrop>())
        + size(chunk.glyphs.len(), size_of::<Glyph>())
        + size(chunk.coords.len(), size_of::<i16>())
}
