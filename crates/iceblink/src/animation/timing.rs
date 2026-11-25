//! How an animation's time maps to progress.

use crate::animation::Easing;

/// How an animation's time maps to iterations and progress, as in Web Animations Level 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    /// The length of one iteration, in nanoseconds.
    pub duration_ns: u64,
    /// The time from the start to the first iteration, in nanoseconds.
    pub delay_ns: i64,
    /// The number of iterations. `f32::INFINITY` repeats for ever.
    pub iterations: f32,
    /// Which iterations run backwards.
    pub direction: Direction,
    /// Whether the end values apply before and after the active phase.
    pub fill: FillMode,
    /// The easing of each iteration's progress.
    pub easing: Easing,
}

/// Which iterations of an animation run backwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Every iteration forwards.
    Normal,
    /// Every iteration backwards.
    Reverse,
    /// Every second iteration, starting with the second.
    Alternate,
    /// Every second iteration, starting with the first.
    AlternateReverse,
}

/// Whether an animation applies its end values outside its active phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillMode {
    /// Neither before nor after.
    None,
    /// The end value after the active phase.
    Forwards,
    /// The start value before the active phase.
    Backwards,
    /// The start value before and the end value after the active phase.
    Both,
}
