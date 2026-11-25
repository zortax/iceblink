//! The resources a vector rasteriser gives a frame's passes.
//!
//! A rasteriser places every pass of a frame's vector plan in a scratch target and groups passes
//! into rounds whose passes fit the scratch together.

mod error;
mod pack;
mod plan;
mod target;

pub use self::error::VectorError;
pub use self::pack::{Capacity, Layering, Placement};
pub use self::plan::{ResourcedPass, ResourcedPlan};
pub use self::target::VectorTarget;
