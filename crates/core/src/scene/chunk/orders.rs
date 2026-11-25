//! The orders of a lowering's instances among themselves.

use std::sync::Arc;

use iceblink::Matrix4;
use smallvec::SmallVec;

use crate::scene::chunk::Lane;

/// The orders of a lowering's instances among themselves, and the placement they were computed
/// under.
///
/// The orders stay valid while every space the instances are drawn in moves by one shared
/// whole-pixel translation.
#[derive(Debug)]
pub(crate) struct LocalOrders {
    /// The revision of the lowering the orders belong to.
    pub(crate) revision: u64,
    /// The chunk offset the orders were computed at.
    pub(crate) offset: [f32; 2],
    /// Each frame space slot the instances are drawn in, with its world matrix then.
    pub(crate) spaces: SmallVec<[(u32, Matrix4); 1]>,
    pub(crate) lanes: LaneOrders,
}

/// The order of every instance of a lowering among the lowering's instances, counting from one.
#[derive(Clone, Debug, Default)]
pub(crate) struct LaneOrders {
    /// Where each lane's orders start in `orders`, by lane.
    pub(crate) starts: [u32; Lane::COUNT],
    /// The orders by lane, and then by index in the lane.
    pub(crate) orders: Arc<[u32]>,
}

impl LaneOrders {
    /// Returns the order of instance `index` of `lane`, or one when it has none.
    pub(crate) fn of(&self, lane: Lane, index: u32) -> u32 {
        let at = self.starts[lane as usize] + index;
        self.orders.get(at as usize).copied().unwrap_or(1)
    }
}
