//! External quads and backdrops in the sweep.

use iceblink::DeviceRect;

use crate::plan::planner::records::Single;
use crate::plan::planner::sweep::Context;
use crate::plan::planner::{PlanStats, Planner};

impl Planner {
    /// Culls, orders and records the external quad at `index` of `Frame::externals`.
    pub(super) fn external(
        &mut self,
        index: u32,
        ink: DeviceRect,
        context: Context<'_>,
        stats: &mut PlanStats,
    ) {
        if !context.cull.drawn(ink) {
            stats.culled += 1;
            return;
        }

        let order = self.tree.insert(ink);
        self.records.externals.push(Single { order, index, ink });
        self.coalescer.occluder(ink, context.input.overlap);
        stats.drawn += 1;
    }

    /// Orders and records the backdrop at `index` of `Frame::backdrops`, which is always drawn.
    pub(super) fn backdrop(
        &mut self,
        index: u32,
        ink: DeviceRect,
        context: Context<'_>,
        stats: &mut PlanStats,
    ) {
        let order = self.tree.insert(ink);
        self.records.backdrops.push(Single { order, index, ink });
        self.coalescer.occluder(ink, context.input.overlap);
        stats.drawn += 1;
    }
}
