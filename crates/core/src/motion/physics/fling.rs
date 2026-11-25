//! The exponential decay of a fling.
//!
//! A fling released at velocity `v` has moved `v (1 - r^s) / -ln r` after `s` seconds, at
//! velocity `v r^s`, with `r = DECAY`.

/// The share of a fling's velocity left after one second.
pub const DECAY: f64 = 0.002;

/// Returns how far a fling released at `velocity` has moved after `seconds`.
pub fn distance(velocity: f64, seconds: f64) -> f64 {
    velocity * (1.0 - DECAY.powf(seconds)) / -DECAY.ln()
}

/// Returns the velocity of a fling released at `velocity`, after `seconds`.
pub fn velocity(velocity: f64, seconds: f64) -> f64 {
    velocity * DECAY.powf(seconds)
}

/// Returns how many seconds after its release a fling at `speed` slows to `rest_speed`, or zero
/// when it starts no faster.
pub fn stop_time(speed: f64, rest_speed: f64) -> f64 {
    let speed = speed.abs();
    if speed <= rest_speed {
        return 0.0;
    }
    (rest_speed / speed).ln() / DECAY.ln()
}
