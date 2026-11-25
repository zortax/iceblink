//! Taking one batch off the sorted kinds.

use iceblink::{Batch, BatchKind, DeviceRect};

use crate::plan::batch::DrawKind;
use crate::plan::batch::merge::{Sequences, SortKey, lane_of, singles};
use crate::plan::planner::records::LaneRecords;

/// Returns the batch starting at `start` of `kind`, and where the kind is consumed to.
///
/// The batch's ink is cut to `cut` when it is set.
pub(super) fn take(
    sequences: &Sequences<'_>,
    kind: DrawKind,
    start: usize,
    limit: SortKey,
    cut: Option<DeviceRect>,
) -> (Batch, usize) {
    if let Some(lane) = lane_of(kind) {
        return take_run(sequences.records.lane(lane), kind, start, limit, cut);
    }

    let one = |kind: BatchKind, ink: DeviceRect| {
        let batch = Batch {
            kind,
            range: 0..0,
            ink: cut_to(ink, cut),
        };
        (batch, start + 1)
    };

    if kind == DrawKind::Path {
        let pass = &sequences.passes[start];
        return one(BatchKind::Paths { pass: start as u32 }, pass.region);
    }

    let single = singles(sequences.records, kind)
        .get(start)
        .expect("the merge peeked this position");
    let batch_kind = match kind {
        DrawKind::External => BatchKind::External(single.index),
        DrawKind::Backdrop => BatchKind::Backdrop(single.index),
        DrawKind::GroupBegin => BatchKind::GroupBegin(single.index),
        _ => BatchKind::GroupEnd(single.index),
    };
    one(batch_kind, single.ink)
}

/// Returns the longest run of `lane` from `start` that sorts below `limit` and keeps one binding.
fn take_run(
    lane: &LaneRecords,
    kind: DrawKind,
    start: usize,
    limit: SortKey,
    cut: Option<DeviceRect>,
) -> (Batch, usize) {
    let key = lane.key_at(start);
    let binds = binding(kind);
    let mut end = start;
    let mut ink: Option<DeviceRect> = None;
    while let Some(order) = lane.order_at(end) {
        if (order, kind) >= limit {
            break;
        }
        let here = lane.key_at(end);
        if (binds[0] && here[0] != key[0]) || (binds[1] && here[1] != key[1]) {
            break;
        }
        let placed = lane.placed_at(end).ink;
        ink = Some(ink.map_or(placed, |held| held.union(placed)));
        end += 1;
    }

    let batch_kind = match kind {
        DrawKind::Shadow => BatchKind::Shadows,
        DrawKind::Quad => BatchKind::Quads,
        DrawKind::ShaderQuad => BatchKind::ShaderQuads {
            shader: key[0],
            params: key[1],
        },
        DrawKind::Marks => BatchKind::Marks,
        DrawKind::Line => BatchKind::Lines,
        DrawKind::Mono => BatchKind::Mono { page: key[0] },
        DrawKind::Subpixel => BatchKind::Subpixel { page: key[0] },
        _ => BatchKind::Color { page: key[0] },
    };
    let batch = Batch {
        kind: batch_kind,
        range: start as u32..end as u32,
        ink: ink.map_or(DeviceRect::ZERO, |ink| cut_to(ink, cut)),
    };
    (batch, end)
}

fn cut_to(ink: DeviceRect, cut: Option<DeviceRect>) -> DeviceRect {
    match cut {
        Some(cut) => ink.intersection(cut).unwrap_or(DeviceRect::ZERO),
        None => ink,
    }
}

/// Returns which words of the key a batch of `kind` keeps equal.
fn binding(kind: DrawKind) -> [bool; 2] {
    match kind {
        DrawKind::Mono | DrawKind::Subpixel | DrawKind::Color => [true, false],
        DrawKind::ShaderQuad => [true, true],
        _ => [false, false],
    }
}
