//! Vector passes: which paths each pass rasterises and where its composite is drawn.
//!
//! A pass rasterises a group of paths into scratch memory, and one composite draws the result at
//! the pass's position in paint order. The planner applies these rules, in order:
//!
//! 0. A path whose clip chain holds a mask gets a pass of its own.
//! 1. Paths whose ink misses the draw damage are dropped.
//! 2. The others are swept in paint order and accumulate into the current pass, whose clip is the
//!    deepest chain they share. The rest of each path's chain is its residual.
//! 3. A new pass starts when a drawn non-path instance that came after a path of the current pass
//!    overlaps that path's ink. [`Overlap`] selects how the overlap is read.
//! 4. A group boundary ends the current pass.
//! 5. A finished pass whose composite cannot sit above every one of its paths and below
//!    everything painted over any of them is split into one pass per path.
//!
//! The composite of a pass takes the highest order among its paths. An instance that comes after
//! a pass is closed and overlaps one of its paths sorts above the composite.
//!
//! A pass outside every group is cut to the bounding box of the draw damage, unless the frame
//! holds a backdrop or the input asks for uncut passes. A pass inside a group keeps its whole
//! region.

mod cells;
mod clips;
pub(crate) mod coalesce;
mod overlap;
pub mod region;
mod trap;
mod uncut;
mod warning;

pub use self::clips::ClipChains;
pub use self::overlap::{Intervening, Overlap};
pub use self::uncut::uncut;
pub use self::warning::PassWarning;
