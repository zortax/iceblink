//! Easing functions.

use std::sync::Arc;

/// A function from progress in `[0, 1]` to eased progress.
#[derive(Clone, Debug, PartialEq)]
pub enum Easing {
    /// The progress unchanged.
    Linear,
    /// A cubic Bézier from `(0, 0)` to `(1, 1)`, with control points `[x1, y1, x2, y2]`.
    ///
    /// `x1` and `x2` are clamped to `[0, 1]`.
    CubicBezier([f32; 4]),
    /// A step function of `count` equal steps.
    Steps {
        /// The number of steps.
        count: u32,
        /// Where the jumps sit.
        jump: StepJump,
    },
    /// A piecewise-linear function through points `(input, output)` with non-decreasing inputs.
    LinearPoints(Arc<[[f32; 2]]>),
    /// The position of a damped mass that starts at 0 and settles at 1, over the iteration's
    /// duration in seconds.
    Spring {
        /// The mass the spring moves.
        mass: f32,
        /// The stiffness of the spring.
        stiffness: f32,
        /// The damping coefficient.
        damping: f32,
        /// The velocity at the start.
        velocity: f32,
    },
}

/// Where the jumps of a step easing sit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepJump {
    /// At the start of each step.
    Start,
    /// At the end of each step.
    End,
    /// Between steps only.
    None,
    /// At both ends and between steps.
    Both,
}
