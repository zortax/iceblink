//! Batching: the draw sequence of a frame.
//!
//! Every lane is sorted by draw order, then by binding. The planner merges the lanes, the vector
//! pass composites and the objects drawn one at a time, sorted by draw order and then by
//! [`DrawKind`]. A batch is a maximal run of one kind that no other kind's next object must
//! precede.

mod kind;
pub(crate) mod merge;

pub use self::kind::DrawKind;
