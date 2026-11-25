//! Draw-order assignment.
//!
//! Each instance takes the lowest order that puts it above every instance it overlaps on the
//! device, so two instances at equal order never overlap. A frame sorts its lanes by order first,
//! and the sequence of instances inside one order is free to serve batching.

mod local;
pub mod sweep;
mod tree;

pub use self::local::local_orders;
pub use self::tree::BoundsTree;

/// A draw order. Higher orders are drawn later, and the lowest assigned order is one.
pub type DrawOrder = u32;
