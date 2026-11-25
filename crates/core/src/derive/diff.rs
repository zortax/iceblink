//! Comparing a re-recorded chunk with the chunk it replaces, item by item.

use std::sync::Arc;

use bytemuck::{Pod, bytes_of};
use iceblink::{ItemKind, MarkPayload, PathItem, RecordedChunk};

/// Reports whether two chunks have the same props and chunk-local clips.
pub(crate) fn same_frame(old: &RecordedChunk, new: &RecordedChunk) -> bool {
    old.props == new.props && old.local_clips == new.local_clips
}

/// Reports whether the items at `position` of both chunks are identical.
///
/// Items are identical when they have the same kind, equal bytes and equal side data. Glyphs and
/// coordinates are compared by value, payloads and filter chains by address or content, and path
/// items field by field with their shared geometry compared by address. A position present in only
/// one chunk is never identical.
pub(crate) fn same_item(old: &RecordedChunk, new: &RecordedChunk, position: usize) -> bool {
    let (Some(&a), Some(&b)) = (old.ops.get(position), new.ops.get(position)) else {
        return false;
    };
    if a.kind != b.kind {
        return false;
    }

    let (x, y) = (a.index as usize, b.index as usize);
    match a.kind {
        ItemKind::QUAD => pod(&old.quads, &new.quads, x, y),
        ItemKind::SHADER_QUAD => pod(&old.shader_quads, &new.shader_quads, x, y),
        ItemKind::SHADOW => pod(&old.shadows, &new.shadows, x, y),
        ItemKind::LINE => pod(&old.lines, &new.lines, x, y),
        ItemKind::EXTERNAL => pod(&old.externals, &new.externals, x, y),
        ItemKind::GLYPH_RUN => glyph_runs(old, new, x, y),
        ItemKind::IMAGE => pod(&old.images, &new.images, x, y),
        ItemKind::MARKS => marks(old, new, x, y),
        ItemKind::PATH => match (old.paths.get(x), new.paths.get(y)) {
            (Some(a), Some(b)) => paths(a, b),
            _ => false,
        },
        ItemKind::BACKDROP => backdrops(old, new, x, y),
        _ => false,
    }
}

fn pod<T: Pod>(old: &[T], new: &[T], a: usize, b: usize) -> bool {
    match (old.get(a), new.get(b)) {
        (Some(a), Some(b)) => bytes_of(a) == bytes_of(b),
        _ => false,
    }
}

/// Reports whether the glyph runs at `index` of both chunks are identical, as `same_item`
/// compares them.
pub(crate) fn same_run(old: &RecordedChunk, new: &RecordedChunk, index: usize) -> bool {
    glyph_runs(old, new, index, index)
}

fn glyph_runs(old: &RecordedChunk, new: &RecordedChunk, a: usize, b: usize) -> bool {
    let (Some(x), Some(y)) = (old.glyph_runs.get(a), new.glyph_runs.get(b)) else {
        return false;
    };
    // The ranges may sit at different places in the two arenas, so the bytes are compared with the
    // ranges taken out.
    let mut x_bare = *x;
    let mut y_bare = *y;
    x_bare.glyphs = Default::default();
    y_bare.glyphs = Default::default();
    x_bare.coords = Default::default();
    y_bare.coords = Default::default();
    if bytes_of(&x_bare) != bytes_of(&y_bare) {
        return false;
    }

    let glyphs = old
        .glyphs
        .get(x.glyphs.indices())
        .zip(new.glyphs.get(y.glyphs.indices()));
    let coords = old
        .coords
        .get(x.coords.indices())
        .zip(new.coords.get(y.coords.indices()));
    match (glyphs, coords) {
        (Some((ga, gb)), Some((ca, cb))) => {
            bytemuck::cast_slice::<_, u8>(ga) == bytemuck::cast_slice::<_, u8>(gb) && ca == cb
        }
        _ => false,
    }
}

fn marks(old: &RecordedChunk, new: &RecordedChunk, a: usize, b: usize) -> bool {
    let (Some(x), Some(y)) = (old.marks.get(a), new.marks.get(b)) else {
        return false;
    };
    let mut x_bare = *x;
    let mut y_bare = *y;
    x_bare.payload = 0;
    y_bare.payload = 0;
    if bytes_of(&x_bare) != bytes_of(&y_bare) {
        return false;
    }

    match (
        old.payloads.get(x.payload as usize),
        new.payloads.get(y.payload as usize),
    ) {
        (Some(p), Some(q)) => Arc::ptr_eq(p, q) || same_payload(p, q),
        _ => false,
    }
}

fn same_payload(a: &MarkPayload, b: &MarkPayload) -> bool {
    bytemuck::cast_slice::<_, u8>(&a.discs) == bytemuck::cast_slice::<_, u8>(&b.discs)
        && bytemuck::cast_slice::<_, u8>(&a.boxes) == bytemuck::cast_slice::<_, u8>(&b.boxes)
        && bytemuck::cast_slice::<_, u8>(&a.vertices) == bytemuck::cast_slice::<_, u8>(&b.vertices)
        && bytemuck::cast_slice::<_, u8>(&a.placements)
            == bytemuck::cast_slice::<_, u8>(&b.placements)
        && a.shapes.len() == b.shapes.len()
        && a.shapes
            .iter()
            .zip(&b.shapes)
            .all(|(p, q)| Arc::ptr_eq(p, q) || p == q)
}

fn paths(a: &PathItem, b: &PathItem) -> bool {
    Arc::ptr_eq(&a.path, &b.path)
        && a.placement == b.placement
        && a.brush == b.brush
        && a.fill == b.fill
        && a.stroke == b.stroke
        && Arc::ptr_eq(&a.clips, &b.clips)
        && a.clip == b.clip
        && a.space == b.space
}

fn backdrops(old: &RecordedChunk, new: &RecordedChunk, a: usize, b: usize) -> bool {
    let (Some(x), Some(y)) = (old.backdrops.get(a), new.backdrops.get(b)) else {
        return false;
    };
    let mut x_bare = *x;
    let mut y_bare = *y;
    x_bare.filters = 0;
    y_bare.filters = 0;
    if bytes_of(&x_bare) != bytes_of(&y_bare) {
        return false;
    }

    match (
        old.filters.get(x.filters as usize),
        new.filters.get(y.filters as usize),
    ) {
        (Some(p), Some(q)) => Arc::ptr_eq(p, q) || p == q,
        _ => false,
    }
}
