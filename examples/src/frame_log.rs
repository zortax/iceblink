//! The switch for a line of statistics per frame.

/// The environment variable that turns on a line of statistics for every frame drawn.
///
/// `1` turns the lines on. They are off without the variable.
pub const FRAME_LOG_VARIABLE: &str = "ICEBLINK_FRAME_LOG";

/// Reports whether [`FRAME_LOG_VARIABLE`] turns on the per-frame lines.
pub fn logs_frames() -> bool {
    std::env::var(FRAME_LOG_VARIABLE).is_ok_and(|value| value == "1")
}
