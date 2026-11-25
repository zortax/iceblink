//! The overscroll bounce: how far a pull past the end displaces content, and how the
//! displacement returns.

/// Returns the displacement of a pull `pull` past the end of a port whose extent on the axis is
/// `extent`, as `e (1 - 1 / (0.55 x / e + 1))` with `x = |pull|` and `e` the extent, at least 1.
///
/// The displacement has the sign of the pull.
pub fn displacement(pull: f64, extent: f64) -> f64 {
    let extent = extent.max(1.0);
    let x = pull.abs();
    (extent * (1.0 - 1.0 / (0.55 * x / extent + 1.0))).copysign(pull)
}

/// Returns the pull that displaces content by `displacement`, which inverts [`displacement`].
///
/// A displacement of the whole extent or more maps to an infinite pull.
pub fn pull(displacement: f64, extent: f64) -> f64 {
    let extent = extent.max(1.0);
    let d = displacement.abs();
    if d >= extent {
        return f64::INFINITY.copysign(displacement);
    }
    (extent * (1.0 / (1.0 - d / extent) - 1.0) / 0.55).copysign(displacement)
}

/// Returns the displacement and its velocity `seconds` after a return started at displacement
/// `start` and velocity `speed`, as `(d₀ + 0.31 v₀ s) e^(-12.5 s)`.
pub fn returning(start: f64, speed: f64, seconds: f64) -> (f64, f64) {
    let decay = (-12.5 * seconds).exp();
    let linear = start + 0.31 * speed * seconds;
    (linear * decay, (0.31 * speed - 12.5 * linear) * decay)
}
