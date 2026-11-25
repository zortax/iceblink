//! How a driver paces its frames.

/// The refresh interval a driver assumes by default, 1/60 s.
pub const DEFAULT_INTERVAL_NS: u64 = 16_666_667;

/// How a driver paces its frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriverOptions {
    /// Who decides when frames are built.
    pub pacing: Pacing,
    /// The refresh interval assumed while no other source gives one.
    pub interval_ns: u64,
    /// The time a paced driver reserves between latching a frame and its display time when the
    /// presenter has no deadlines, or `None` to measure it from the frames it draws.
    ///
    /// A fixed budget longer than the interval counts as one interval.
    pub budget_ns: Option<u64>,
}

/// Who decides when a driver builds frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pacing {
    /// The driver latches each frame at a deadline before the display time it is built for.
    Paced,
    /// The driver builds frames only at the clocks of `CompositorClient::step`.
    Stepped,
}

impl Default for DriverOptions {
    fn default() -> Self {
        Self {
            pacing: Pacing::Paced,
            interval_ns: DEFAULT_INTERVAL_NS,
            budget_ns: None,
        }
    }
}

impl DriverOptions {
    /// Returns the options of a driver stepped at the clocks of `CompositorClient::step`.
    pub fn stepped() -> Self {
        Self {
            pacing: Pacing::Stepped,
            ..Self::default()
        }
    }
}
