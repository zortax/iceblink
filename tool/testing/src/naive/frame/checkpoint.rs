//! Taking back what a group lowered, when the group draws nothing.

use iceblink::{FrameCounters, Lane};

use crate::naive::frame::builder::Builder;

/// The lengths of every list a group appends to, and the counters before it.
pub(crate) struct Checkpoint {
    lanes: [(usize, usize); 8],
    paths: usize,
    externals: usize,
    backdrops: usize,
    groups: usize,
    batches: usize,
    planned: usize,
    passes: usize,
    union_marks: usize,
    counters: FrameCounters,
}

impl Checkpoint {
    pub(crate) fn take(builder: &Builder<'_>) -> Self {
        let frame = &builder.frame;
        let lanes = &frame.lanes;
        Self {
            lanes: [
                lengths(&lanes.quads),
                lengths(&lanes.shader_quads),
                lengths(&lanes.shadows),
                lengths(&lanes.lines),
                lengths(&lanes.mono),
                lengths(&lanes.subpixel),
                lengths(&lanes.color),
                lengths(&lanes.marks),
            ],
            paths: frame.paths.len(),
            externals: frame.externals.len(),
            backdrops: frame.backdrops.len(),
            groups: frame.groups.len(),
            batches: frame.batches.len(),
            planned: frame.vector_plan.items.len(),
            passes: frame.vector_plan.passes.len(),
            union_marks: builder.union_marks.len(),
            counters: builder.counters,
        }
    }

    /// Truncates every list to its length at the checkpoint and restores the counters.
    ///
    /// Frame slots, pages and shaders the group used are kept.
    pub(crate) fn restore(self, builder: &mut Builder<'_>) {
        let frame = &mut *builder.frame;
        let lanes = &mut frame.lanes;
        truncate(&mut lanes.quads, self.lanes[0]);
        truncate(&mut lanes.shader_quads, self.lanes[1]);
        truncate(&mut lanes.shadows, self.lanes[2]);
        truncate(&mut lanes.lines, self.lanes[3]);
        truncate(&mut lanes.mono, self.lanes[4]);
        truncate(&mut lanes.subpixel, self.lanes[5]);
        truncate(&mut lanes.color, self.lanes[6]);
        truncate(&mut lanes.marks, self.lanes[7]);
        frame.paths.truncate(self.paths);
        frame.externals.truncate(self.externals);
        frame.backdrops.truncate(self.backdrops);
        frame.groups.truncate(self.groups);
        frame.batches.truncate(self.batches);
        frame.vector_plan.items.truncate(self.planned);
        frame.vector_plan.passes.truncate(self.passes);
        builder.union_marks.truncate(self.union_marks);
        builder.counters = self.counters;
    }
}

fn lengths<T>(lane: &Lane<T>) -> (usize, usize) {
    (lane.items.len(), lane.sources.len())
}

fn truncate<T>(lane: &mut Lane<T>, (items, sources): (usize, usize)) {
    lane.items.truncate(items);
    lane.sources.truncate(sources);
}
