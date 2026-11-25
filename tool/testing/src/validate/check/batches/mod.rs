//! The draw sequence: batch ranges, lane coverage, bindings, and the objects batches draw.

mod bindings;
mod nesting;
mod objects;

use iceblink::{Batch, BatchKind, Frame};

use crate::validate::Rule;
use crate::validate::check::Inspect;
use crate::validate::check::resolve::Lanes;

/// The lanes batches draw from, in the order of `LANES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaneId {
    Quads,
    ShaderQuads,
    Shadows,
    Lines,
    Mono,
    Subpixel,
    Color,
    Marks,
}

const LANES: [LaneId; 8] = [
    LaneId::Quads,
    LaneId::ShaderQuads,
    LaneId::Shadows,
    LaneId::Lines,
    LaneId::Mono,
    LaneId::Subpixel,
    LaneId::Color,
    LaneId::Marks,
];

impl LaneId {
    fn of(kind: BatchKind) -> Option<Self> {
        Some(match kind {
            BatchKind::Quads => Self::Quads,
            BatchKind::ShaderQuads { .. } => Self::ShaderQuads,
            BatchKind::Shadows => Self::Shadows,
            BatchKind::Lines => Self::Lines,
            BatchKind::Mono { .. } => Self::Mono,
            BatchKind::Subpixel { .. } => Self::Subpixel,
            BatchKind::Color { .. } => Self::Color,
            BatchKind::Marks => Self::Marks,
            BatchKind::Paths { .. }
            | BatchKind::External(_)
            | BatchKind::Backdrop(_)
            | BatchKind::GroupBegin(_)
            | BatchKind::GroupEnd(_) => return None,
        })
    }

    fn sources(self, frame: &Frame) -> usize {
        let lanes = &frame.lanes;
        match self {
            Self::Quads => lanes.quads.drawn(),
            Self::ShaderQuads => lanes.shader_quads.drawn(),
            Self::Shadows => lanes.shadows.drawn(),
            Self::Lines => lanes.lines.drawn(),
            Self::Mono => lanes.mono.drawn(),
            Self::Subpixel => lanes.subpixel.drawn(),
            Self::Color => lanes.color.drawn(),
            Self::Marks => lanes.marks.drawn(),
        }
    }

    fn index(self) -> usize {
        LANES.iter().position(|lane| *lane == self).unwrap_or(0)
    }
}

/// Checks the draw sequence, and returns how many groups are open at each batch.
pub(crate) fn check(inspect: &mut Inspect<'_>, lanes: &Lanes<'_>) -> Vec<usize> {
    let frame = inspect.frame;

    // The next source of each lane that a batch must start at.
    let mut cursors = [0usize; LANES.len()];
    for (index, batch) in frame.batches.iter().enumerate() {
        match LaneId::of(batch.kind) {
            Some(lane) => instanced(inspect, lanes, index, batch, lane, &mut cursors),
            None if batch.range.start != batch.range.end => inspect.push(
                Rule::BatchRanges,
                format!(
                    "batch {index} draws one object but has range {:?}",
                    batch.range
                ),
            ),
            None => {}
        }
    }

    for lane in LANES {
        let len = lane.sources(frame);
        if cursors[lane.index()] != len {
            inspect.push(
                Rule::BatchCoverage,
                format!(
                    "{lane:?} sources {}..{len} are drawn by no batch",
                    cursors[lane.index()]
                ),
            );
        }
    }

    objects::check(inspect);
    nesting::check(inspect)
}

fn instanced(
    inspect: &mut Inspect<'_>,
    lanes: &Lanes<'_>,
    index: usize,
    batch: &Batch,
    lane: LaneId,
    cursors: &mut [usize; LANES.len()],
) {
    let len = lane.sources(inspect.frame);
    let (start, end) = (batch.range.start as usize, batch.range.end as usize);
    if start > end || end > len {
        inspect.push(
            Rule::BatchRanges,
            format!("batch {index} draws {lane:?} {start}..{end} of {len}"),
        );
        // The sources it reaches stay covered, so one bad range is reported once.
        let cursor = &mut cursors[lane.index()];
        *cursor = (*cursor).max(end.min(len));
        return;
    }

    let cursor = &mut cursors[lane.index()];
    if start != *cursor {
        inspect.push(
            Rule::BatchCoverage,
            format!(
                "batch {index} draws {lane:?} from {start}, where the lane continues at {cursor}"
            ),
        );
    }
    *cursor = (*cursor).max(end);

    bindings::check(inspect, lanes, index, batch.kind, start..end);
}
