//! The state a driver publishes for its clients to read.

use crate::{Counters, MemoryReport, RenderCapabilities, TargetPoolReport, VectorStatus};

/// Who runs a driver's frames, which decides what its client calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverPacing {
    /// The driver latches and builds its frames on a thread of its own, and the client neither
    /// steps nor pumps it.
    SelfPaced,
    /// The driver latches its frames on its own and builds them on the client's thread, in
    /// `pump`, which the client calls when `DriverStats::next_due_ns` arrives.
    Pumped,
    /// The driver builds frames only in `step`, which the client calls with its own clocks
    /// while `DriverStats::next_due_ns` is set.
    Stepped,
}

/// A snapshot of a driver and the compositor it runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriverStats {
    /// When the driver latches its next frame, on the frame clock, or `None` when no output
    /// wants a frame.
    pub next_due_ns: Option<u64>,
    /// When the next frame to latch is predicted to be shown on the display, on the frame clock,
    /// or `None` when no output wants a frame.
    pub next_display_ns: Option<u64>,
    /// When the frame due latches, on the frame clock, when a commit queued at the snapshot goes
    /// into it.
    ///
    /// The value is `None` when no frame is due, when the frame due is a later one the compositor
    /// wants for itself, such as one that brings a node to rest, and for a stepped driver. A commit
    /// then latches immediately or at the next refresh.
    pub commit_due_ns: Option<u64>,
    /// When the frame of `commit_due_ns` is predicted to be shown on the display, on the frame
    /// clock, or `None` when `commit_due_ns` is.
    pub commit_display_ns: Option<u64>,
    /// The frames built for every output since the driver started.
    pub frames_built: u64,
    /// The steps a stepped driver applied since it started.
    ///
    /// The snapshot reflects every step counted here.
    pub steps: u64,
    /// The counters of the compositor's most recent commit and frame.
    pub counters: Counters,
}

/// A snapshot of one output and the renderer that draws it.
///
/// An output whose renderer could not be built reports `failed`, and the capabilities of
/// `RenderCapabilities::MINIMAL` and nothing held for the renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputStatus {
    /// The renderer's capabilities, which decide what a frame holds.
    pub caps: RenderCapabilities,
    /// The renderer's path rasteriser.
    pub vector: VectorStatus,
    /// The device memory the renderer holds.
    pub memory: MemoryReport,
    /// The renderer's pooled group targets.
    pub target_pool: TargetPoolReport,
    /// The frames built for the output since it was created.
    pub frames_built: u64,
    /// Whether the output's renderer could not be built, so the output draws nothing.
    pub failed: bool,
    /// Whether the presenter's last frames timed out waiting for an image, since the last frame
    /// it presented.
    pub starved: bool,
    /// Whether the platform reported the output hidden, since the last frame it presented.
    pub occluded: bool,
}
