//! Merging the sorted kinds into the draw sequence.

mod covers;
mod take;

use iceblink::{Batch, BatchKind, DeviceRect, PlannedPass};

use crate::plan::batch::DrawKind;
use crate::plan::cover::Covers;
use crate::plan::emission::LaneKind;
use crate::plan::order::DrawOrder;
use crate::plan::pixels;
use crate::plan::planner::records::{Records, Singles};

/// A position in the merged sequence: draw order first, then kind.
pub(super) type SortKey = (DrawOrder, DrawKind);

/// Everything the merge reads.
pub(crate) struct Sequences<'a> {
    pub(crate) records: &'a Records,
    pub(crate) passes: &'a [PlannedPass],
    /// The composite order of each pass, parallel to `passes`.
    pub(crate) pass_orders: &'a [DrawOrder],
    /// The opaque cover rectangles, by cover index.
    pub(crate) covers: &'a [DeviceRect],
    pub(crate) size: [u32; 2],
    /// Whether the frame may carry opaque covers.
    pub(crate) covers_allowed: bool,
}

/// Writes the draw sequence into `batches` and the frame's opaque covers into `covers`.
///
/// Each step takes the longest run of one kind that sorts below every other kind's next object.
/// A run of sprites holds one page, and a run of shader quads one shader and parameter slot.
/// Opaque covers are taken from the quad batches before the first group, when the frame allows
/// covers. Inks are cut to the output outside groups, and kept whole inside them, where a group's
/// target can reach past the output.
pub(crate) fn merge(sequences: &Sequences<'_>, batches: &mut Vec<Batch>, covers: &mut Covers) {
    batches.clear();
    covers.clear();

    let output = pixels::output(sequences.size);
    let mut cursors = [0usize; DrawKind::ALL.len()];
    let mut covering = sequences.covers_allowed;
    let mut depth = 0u32;
    loop {
        let (best, second) = two_lowest(sequences, &cursors);
        let Some((_, kind)) = best else { break };
        let limit = second.unwrap_or((DrawOrder::MAX, DrawKind::GroupEnd));
        let start = cursors[kind as usize];
        let (batch, end) = take::take(
            sequences,
            kind,
            start,
            limit,
            (depth == 0).then_some(output),
        );

        match kind {
            DrawKind::GroupBegin => depth += 1,
            DrawKind::GroupEnd => depth = depth.saturating_sub(1),
            _ => {}
        }

        if kind == DrawKind::Quad && covering {
            covers::collect(sequences, start..end, batches.len() as u32, covers);
        }
        covering &= kind != DrawKind::GroupBegin;

        cursors[kind as usize] = end;
        batches.push(batch);
    }
}

/// Returns the sort key of the next object of `kind` at `cursor`.
fn peek(sequences: &Sequences<'_>, kind: DrawKind, cursor: usize) -> Option<SortKey> {
    let records = sequences.records;
    let order = match lane_of(kind) {
        Some(lane) => records.lane(lane).order_at(cursor),
        None => match kind {
            DrawKind::Path => sequences.pass_orders.get(cursor).copied(),
            _ => singles(records, kind)
                .get(cursor)
                .map(|single| single.order),
        },
    };
    order.map(|order| (order, kind))
}

/// Returns the two lowest waiting keys.
// A scan rather than a sort: finding the two smallest of thirteen in one pass is cheaper, and the
// second smallest is exactly the bound the winning run may not cross.
fn two_lowest(sequences: &Sequences<'_>, cursors: &[usize]) -> (Option<SortKey>, Option<SortKey>) {
    let mut best: Option<SortKey> = None;
    let mut second: Option<SortKey> = None;
    for kind in DrawKind::ALL {
        let Some(candidate) = peek(sequences, kind, cursors[kind as usize]) else {
            continue;
        };
        if best.is_none_or(|held| candidate < held) {
            second = best;
            best = Some(candidate);
        } else if second.is_none_or(|held| candidate < held) {
            second = Some(candidate);
        }
    }
    (best, second)
}

/// Returns the lane of an instanced kind.
pub(super) fn lane_of(kind: DrawKind) -> Option<LaneKind> {
    Some(match kind {
        DrawKind::Shadow => LaneKind::Shadows,
        DrawKind::Quad => LaneKind::Quads,
        DrawKind::ShaderQuad => LaneKind::ShaderQuads,
        DrawKind::Marks => LaneKind::Marks,
        DrawKind::Line => LaneKind::Lines,
        DrawKind::Mono => LaneKind::Mono,
        DrawKind::Subpixel => LaneKind::Subpixel,
        DrawKind::Color => LaneKind::Color,
        _ => return None,
    })
}

/// Returns the lane an instanced batch draws from.
pub(crate) fn lane_of_batch(kind: BatchKind) -> Option<LaneKind> {
    Some(match kind {
        BatchKind::Shadows => LaneKind::Shadows,
        BatchKind::Quads => LaneKind::Quads,
        BatchKind::ShaderQuads { .. } => LaneKind::ShaderQuads,
        BatchKind::Marks => LaneKind::Marks,
        BatchKind::Lines => LaneKind::Lines,
        BatchKind::Mono { .. } => LaneKind::Mono,
        BatchKind::Subpixel { .. } => LaneKind::Subpixel,
        BatchKind::Color { .. } => LaneKind::Color,
        _ => return None,
    })
}

/// Returns the objects of a kind drawn one at a time, other than vector passes.
pub(super) fn singles(records: &Records, kind: DrawKind) -> &Singles {
    match kind {
        DrawKind::GroupBegin => &records.begins,
        DrawKind::External => &records.externals,
        DrawKind::Backdrop => &records.backdrops,
        _ => &records.ends,
    }
}
