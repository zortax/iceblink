//! Batch entries merged into the batches of a frame.

use iceblink::{Batch, BatchKind, DeviceRect};

/// One item or group boundary in paint order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Entry {
    Quad,
    ShaderQuad(u32, u32),
    Shadow,
    Line,
    Mono(u32),
    Subpixel(u32),
    Color(u32),
    Marks,
    Path(u32),
    External(u32),
    Backdrop(u32),
    GroupBegin(u32),
    GroupEnd(u32),
}

/// Returns the batches of `entries`, in paint order.
///
/// Consecutive instances of one batch kind share a batch, whose ink is the union of theirs. Every
/// other entry is a batch of its own.
pub(super) fn batches(entries: Vec<(Entry, DeviceRect)>) -> Vec<Batch> {
    let mut positions = [0u32; 8];
    let mut batches: Vec<Batch> = Vec::new();
    for (entry, ink) in entries {
        let (kind, lane) = kind(entry);
        let Some(lane) = lane else {
            batches.push(Batch {
                kind,
                range: 0..0,
                ink,
            });
            continue;
        };

        let position = positions[lane];
        positions[lane] += 1;
        if let Some(last) = batches.last_mut()
            && last.kind == kind
            && last.range.end == position
        {
            last.range.end += 1;
            last.ink = last.ink.union(ink);
            continue;
        }
        batches.push(Batch {
            kind,
            range: position..position + 1,
            ink,
        });
    }
    batches
}

/// Returns the batch kind of `entry`, and the instance lane it draws from, if any.
fn kind(entry: Entry) -> (BatchKind, Option<usize>) {
    match entry {
        Entry::Quad => (BatchKind::Quads, Some(0)),
        Entry::ShaderQuad(shader, params) => (BatchKind::ShaderQuads { shader, params }, Some(1)),
        Entry::Shadow => (BatchKind::Shadows, Some(2)),
        Entry::Line => (BatchKind::Lines, Some(3)),
        Entry::Mono(page) => (BatchKind::Mono { page }, Some(4)),
        Entry::Subpixel(page) => (BatchKind::Subpixel { page }, Some(5)),
        Entry::Color(page) => (BatchKind::Color { page }, Some(6)),
        Entry::Marks => (BatchKind::Marks, Some(7)),
        Entry::Path(pass) => (BatchKind::Paths { pass }, None),
        Entry::External(index) => (BatchKind::External(index), None),
        Entry::Backdrop(index) => (BatchKind::Backdrop(index), None),
        Entry::GroupBegin(index) => (BatchKind::GroupBegin(index), None),
        Entry::GroupEnd(index) => (BatchKind::GroupEnd(index), None),
    }
}
