//! Scroll physics in device pixels: fling decay, the overscroll bounce and the wheel step curve.

pub mod bounce;
pub mod fling;
pub mod step;

/// How close to its target a motion must be to arrive, in device pixels.
pub const ARRIVAL_PX: f64 = 0.5;

/// The refresh interval assumed when none is known, 1/60 s.
pub const FALLBACK_INTERVAL_NS: u64 = 16_666_667;

/// The smallest delta on either axis that moves anything, in device pixels.
pub const DEAD_DELTA: f32 = 0.01;

/// Returns the speed of one device pixel per refresh at a refresh interval of `interval_ns`, in
/// device pixels per second.
///
/// An interval of zero counts as [`FALLBACK_INTERVAL_NS`].
pub fn rest_speed(interval_ns: u64) -> f64 {
    let interval_ns = if interval_ns == 0 {
        FALLBACK_INTERVAL_NS
    } else {
        interval_ns
    };
    1e9 / interval_ns as f64
}

/// Reports whether a motion `away` device pixels off its target at `velocity` device pixels per
/// second is within [`ARRIVAL_PX`] of it and slower than `rest_speed`.
pub fn arrived(away: f64, velocity: f64, rest_speed: f64) -> bool {
    away.abs() < ARRIVAL_PX && velocity.abs() < rest_speed
}
