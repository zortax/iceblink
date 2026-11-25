//! The frame paint slots of the inline colours a recording uses.

use iceblink::{ChunkKey, PaintRef, RecordedChunk};
use smallvec::SmallVec;

use crate::derive::Derive;

/// Takes a frame paint slot for every inline colour of a newly recorded chunk.
pub(super) fn hold_inline(derive: &mut Derive<'_>, key: ChunkKey) {
    let Some(record) = derive.scene.chunks.get_mut(&key) else {
        return;
    };
    let colors = colors(&record.recorded);
    derive.tables.hold_inline(&colors);
    record.derived.inline = colors;
}

/// Returns the inline colours of a recording, each once.
fn colors(chunk: &RecordedChunk) -> SmallVec<[[u8; 4]; 2]> {
    let mut colors = SmallVec::new();
    let mut note = |paint: PaintRef| {
        // Neighbouring items often share a colour.
        if let Some(rgba) = paint.inline_color()
            && colors.last() != Some(&rgba)
        {
            colors.push(rgba);
        }
    };

    for quad in &chunk.quads {
        note(quad.fill);
        note(quad.stroke);
    }
    for quad in &chunk.shader_quads {
        note(quad.fill);
        note(quad.stroke);
    }
    for run in &chunk.glyph_runs {
        note(run.paint);
    }
    for image in &chunk.images {
        note(image.paint);
    }
    for marks in &chunk.marks {
        note(marks.paint);
    }
    for path in &chunk.paths {
        if let Some(fill) = &path.fill {
            note(fill.paint);
        }
        if let Some(stroke) = &path.stroke {
            note(stroke.paint);
        }
    }

    colors.sort_unstable();
    colors.dedup();
    colors
}
