//! Instances in the sweep.

use crate::plan::emission::{Instance, LaneKind};
use crate::plan::marks::UnionMark;
use crate::plan::order::DrawOrder;
use crate::plan::planner::records::Placed;
use crate::plan::planner::sweep::Context;
use crate::plan::planner::{PlanStats, Planner};

impl Planner {
    /// Culls, orders and records an instance outside a block.
    pub(super) fn instance(
        &mut self,
        instance: Instance,
        cover: u32,
        context: Context<'_>,
        stats: &mut PlanStats,
    ) {
        if !context.cull.admits(&instance) {
            stats.culled += 1;
            return;
        }

        let order = self.tree.insert(instance.ink);
        self.record(instance, cover, order, context, stats);
    }

    /// Records a drawn instance at `order`.
    pub(super) fn record(
        &mut self,
        instance: Instance,
        cover: u32,
        order: DrawOrder,
        context: Context<'_>,
        stats: &mut PlanStats,
    ) {
        let lane = instance.lane;
        let push = self.records.lane_mut(lane).push(
            order,
            instance.key,
            Placed {
                source: instance.source,
                ink: instance.ink,
                cover,
            },
        );

        if instance.union && lane == LaneKind::Marks {
            self.unions.push(UnionMark {
                push,
                ink: instance.ink,
                nested: !self.groups.is_empty(),
            });
        }

        self.coalescer.occluder(instance.ink, context.input.overlap);
        stats.drawn += 1;
    }
}
