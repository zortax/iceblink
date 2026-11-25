//! Frame planning: draw order, batching, opaque covers, vector passes and mark bins.
//!
//! The compositor lowers a frame's items into frame kinds and lists what it draws, in paint order,
//! in an [`Emission`]. A [`Planner`] turns that stream into the parts of a
//! [`Frame`](iceblink::Frame) that decide how it is drawn: the draw order of every lane,
//! the batches, the vector plan, the mark plan and the opaque covers.
//!
//! Each instance takes the lowest draw order above every instance it overlaps, from a
//! [`BoundsTree`] over device inks. Instances at equal order never overlap, so each lane is sorted
//! by order and then by its binding, and a batch is a maximal run of one kind.

mod batch;
mod cover;
mod emission;
mod marks;
pub mod order;
pub mod pass;
mod pixels;
mod planner;

pub use crate::plan::batch::DrawKind;
pub use crate::plan::emission::{Emission, Instance, LaneKind};
pub use crate::plan::order::{BoundsTree, DrawOrder, local_orders};
pub use crate::plan::pass::{ClipChains, Overlap, PassWarning};
pub use crate::plan::pixels::{covering, inner_pixels};
pub use crate::plan::planner::{Input, Output, PlanStats, Planner};
