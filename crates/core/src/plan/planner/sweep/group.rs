//! Group boundaries in the sweep.

use iceblink::{DeviceRect, VectorPlan};

use crate::plan::pass::ClipChains;
use crate::plan::planner::Planner;
use crate::plan::planner::records::Single;
use crate::plan::planner::sweep::Context;

impl Planner {
    /// Starts the group at `index` of `Frame::groups`, which writes `bounds`.
    pub(super) fn begin_group(
        &mut self,
        index: u32,
        bounds: DeviceRect,
        context: Context<'_>,
        clips: &mut impl ClipChains,
        vector_plan: &mut VectorPlan,
    ) {
        self.coalescer
            .boundary(true, context.pass, clips, vector_plan);
        self.raise();

        let order = self.tree.insert_above_all(bounds);
        // Content outside the group's bounds still sorts inside the group.
        self.tree.set_order_floor(order);
        self.records.begins.push(Single {
            order,
            index,
            ink: bounds,
        });
        self.groups.push((index, bounds));
    }

    /// Ends the innermost open group.
    pub(super) fn end_group(
        &mut self,
        context: Context<'_>,
        clips: &mut impl ClipChains,
        vector_plan: &mut VectorPlan,
    ) {
        let Some((index, bounds)) = self.groups.pop() else {
            return;
        };
        self.coalescer
            .boundary(false, context.pass, clips, vector_plan);
        self.raise();

        let order = self.tree.insert_above_all(bounds);
        // One above the end's own order: a later instance at the end's order would sort before it
        // and land inside the closed group.
        self.tree.set_order_floor(order + 1);
        self.records.ends.push(Single {
            order,
            index,
            ink: bounds,
        });
    }
}
