//! The curve a kinetic step follows from where the offset is to its destination.

/// How long a step's curve lasts, in nanoseconds.
pub const DURATION_NS: u64 = 240_000_000;

/// Returns the share of the way a step has come `elapsed_ns` after it started, from 0 to 1.
///
/// The curve is the cubic ease-out `1 - (1 - p)³` at the share `p` of the duration.
pub fn share(elapsed_ns: u64) -> f32 {
    if elapsed_ns >= DURATION_NS {
        return 1.0;
    }
    let left = 1.0 - elapsed_ns as f32 / DURATION_NS as f32;
    1.0 - left * left * left
}
