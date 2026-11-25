//! What renderers and presenters report: capabilities, targets, outcomes, memory, presentation
//! and the path rasteriser in use.

mod adapter;
mod age;
mod capabilities;
mod memory;
mod outcome;
mod present;
mod subpixel;
mod target;
pub mod vector;

pub use self::adapter::{GpuUnavailable, RejectedAdapter};
pub use self::age::BufferAge;
pub use self::capabilities::RenderCapabilities;
pub use self::memory::{MemoryReport, TargetPoolReport};
pub use self::outcome::{FrameOutcome, FrameStats, SkipReason};
pub use self::present::{
    Acquired, PresentCapabilities, PresentFeedback, PresentMode, PresentResult, STALE_DAMAGE,
};
pub use self::subpixel::SubpixelOrder;
pub use self::target::RenderTarget;
