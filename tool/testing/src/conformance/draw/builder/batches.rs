//! The batches a frame draws its entries in.

use iceblink::{Batch, BatchKind, DeviceRect};

/// One thing a frame draws, in paint order.
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

impl Entry {
    /// Returns the kind of batch that draws the entry, and the index of its lane if it has one.
    fn kind(self) -> (BatchKind, Option<usize>) {
        match self {
            Self::Quad => (BatchKind::Quads, Some(0)),
            Self::ShaderQuad(shader, params) => {
                (BatchKind::ShaderQuads { shader, params }, Some(1))
            }
            Self::Shadow => (BatchKind::Shadows, Some(2)),
            Self::Line => (BatchKind::Lines, Some(3)),
            Self::Mono(page) => (BatchKind::Mono { page }, Some(4)),
            Self::Subpixel(page) => (BatchKind::Subpixel { page }, Some(5)),
            Self::Color(page) => (BatchKind::Color { page }, Some(6)),
            Self::Marks => (BatchKind::Marks, Some(7)),
            Self::Path(pass) => (BatchKind::Paths { pass }, None),
            Self::External(index) => (BatchKind::External(index), None),
            Self::Backdrop(index) => (BatchKind::Backdrop(index), None),
            Self::GroupBegin(index) => (BatchKind::GroupBegin(index), None),
            Self::GroupEnd(index) => (BatchKind::GroupEnd(index), None),
        }
    }
}

/// Returns the batches that draw `entries` with their inks, in order.
///
/// Consecutive lane entries of one kind share a batch, and every other entry gets a batch of its
/// own.
pub(super) fn batches(entries: Vec<(Entry, DeviceRect)>) -> Vec<Batch> {
    let mut positions = [0u32; 8];
    let mut batches: Vec<Batch> = Vec::new();
    for (entry, ink) in entries {
        let (kind, lane) = entry.kind();
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
