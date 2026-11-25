//! What the last plan decided, for checks and transcripts.

use crate::plan::batch::DrawKind;
use crate::plan::emission::LaneKind;
use crate::plan::order::DrawOrder;
use crate::plan::order::sweep::{OrderOverlap, Ordered, TooManyAtOneOrder, overlaps};
use crate::plan::planner::Planner;

impl Planner {
    /// Returns the draw orders of the last plan's instances of `lane`, in the lane's draw order.
    pub fn orders(&self, lane: LaneKind) -> impl Iterator<Item = DrawOrder> + '_ {
        self.records.lane(lane).orders()
    }

    /// Returns every pair of drawn objects of the last plan that share a draw order and overlap.
    ///
    /// Group markers and vector passes are left out. An empty result is what the planner
    /// guarantees.
    ///
    /// # Errors
    ///
    /// Fails with [`TooManyAtOneOrder`] when one order holds more than `max` objects.
    pub fn order_overlaps(&self, max: usize) -> Result<Vec<OrderOverlap>, TooManyAtOneOrder> {
        let mut ordered = Vec::new();
        for lane in LaneKind::ALL {
            let kind = DrawKind::of_lane(lane);
            self.records
                .lane(lane)
                .for_each(|order, ink| ordered.push(Ordered { order, kind, ink }));
        }
        for (singles, kind) in [
            (&self.records.externals, DrawKind::External),
            (&self.records.backdrops, DrawKind::Backdrop),
        ] {
            ordered.extend(singles.iter().map(|single| Ordered {
                order: single.order,
                kind,
                ink: single.ink,
            }));
        }
        overlaps(&ordered, max)
    }
}
