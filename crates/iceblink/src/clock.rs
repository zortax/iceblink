//! The frame clock.

use std::sync::OnceLock;

use web_time::Instant;

static EPOCH: OnceLock<Instant> = OnceLock::new();

/// Returns the time on the frame clock, in nanoseconds.
///
/// The frame clock is monotonic and starts at its first reading in the process. Frame clocks,
/// presenter deadlines and feedback, the gesture stamps of a paced driver and `DriverStats` times
/// are measured on it.
pub fn now_ns() -> u64 {
    let epoch = EPOCH.get_or_init(Instant::now);
    u64::try_from(epoch.elapsed().as_nanos()).unwrap_or(u64::MAX)
}
