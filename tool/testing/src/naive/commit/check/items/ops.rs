//! The record order of a chunk's items.

use iceblink::{RecordedChunk, RejectReason};

/// Fails with `InvalidItem` unless the chunk's `ops` refer to every entry of each item array once,
/// in array order.
pub(super) fn check(chunk: &RecordedChunk) -> Result<(), RejectReason> {
    let lengths = [
        chunk.quads.len(),
        chunk.shader_quads.len(),
        chunk.shadows.len(),
        chunk.lines.len(),
        chunk.glyph_runs.len(),
        chunk.images.len(),
        chunk.marks.len(),
        chunk.paths.len(),
        chunk.externals.len(),
        chunk.backdrops.len(),
    ];

    let mut next = [0usize; 10];
    for op in &chunk.ops {
        let kind = usize::from(op.kind.0);
        if !op.kind.is_valid() || kind >= next.len() || op.index as usize != next[kind] {
            return Err(RejectReason::InvalidItem);
        }

        next[kind] += 1;
    }

    if next == lengths {
        Ok(())
    } else {
        Err(RejectReason::InvalidItem)
    }
}
