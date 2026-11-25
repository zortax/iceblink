//! Animations the compositor samples on its own.

mod easing;
mod keyframes;
mod target;
mod timing;

pub use self::easing::{Easing, StepJump};
pub use self::keyframes::{Keyframe, Keyframes};
pub use self::target::{AnimationTarget, FilterParam};
pub use self::timing::{Direction, FillMode, Timing};

/// An animation of one property, which the compositor samples at the present time of every frame.
///
/// The animation overrides the value the client last wrote to its target, and that value returns
/// once the animation ends.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// The property animated.
    pub target: AnimationTarget,
    /// The values the property passes through. Their type matches the target.
    pub keyframes: Keyframes,
    /// How long the animation runs, how often it repeats and how it fills.
    pub timing: Timing,
    /// When the animation's time starts.
    pub start: AnimationStart,
}

/// When an animation's time starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationStart {
    /// At the present time of the first frame that samples the animation.
    Next,
    /// At a time on the frame clock, in nanoseconds.
    At(u64),
}
