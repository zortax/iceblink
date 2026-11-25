//! Blocks in the sweep.

use iceblink::DeviceRect;

use crate::plan::emission::Entry;
use crate::plan::order::DrawOrder;
use crate::plan::planner::sweep::Context;
use crate::plan::planner::{PlanStats, Planner};

impl Planner {
    /// Orders and records the members of one block with one query of the bounds tree.
    pub(super) fn block(&mut self, members: &[Entry], context: Context<'_>, stats: &mut PlanStats) {
        self.survivors.clear();
        let mut union: Option<DeviceRect> = None;
        let mut span: DrawOrder = 1;
        for (offset, member) in members.iter().enumerate() {
            let Entry::Instance { instance, .. } = member else {
                continue;
            };
            if !context.cull.admits(instance) {
                stats.culled += 1;
                continue;
            }
            self.survivors.push(offset as u32);
            union = Some(union.map_or(instance.ink, |held| held.union(instance.ink)));
            span = span.max(instance.local.max(1));
        }
        let Some(union) = union else { return };

        let base = self.tree.insert_block(union, span - 1);
        for offset in 0..self.survivors.len() {
            let Entry::Instance { instance, cover } = members[self.survivors[offset] as usize]
            else {
                continue;
            };
            let order = base + instance.local.max(1) - 1;
            self.record(instance, cover, order, context, stats);
        }
    }
}
