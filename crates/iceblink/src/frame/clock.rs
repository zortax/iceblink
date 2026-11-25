//! The times a frame is built and drawn for.

/// Times on one monotonic clock in nanoseconds, shared by the compositor and the presenter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameClock {
    /// When the frame is built.
    pub now_ns: u64,
    /// When the frame is predicted to be shown on the display.
    pub present_ns: u64,
    /// The display's refresh interval.
    pub interval_ns: u64,
}

/// The times application shaders read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShaderClock {
    /// Seconds from the compositor's start to the frame's present time.
    ///
    /// The compositor's start is the `now_ns` of its first frame build.
    pub time: f32,
    /// Seconds from the previous frame's present time to this frame's, zero on an output's first
    /// frame.
    pub delta: f32,
}
