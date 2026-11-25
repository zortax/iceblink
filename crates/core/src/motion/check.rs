//! Whether an animation is well formed, apart from whether its target exists.

use iceblink::{
    Animation, AnimationTarget, Easing, FilterParam, Keyframe, Keyframes, StepJump, Timing,
};

/// Reports whether `animation` is well formed:
///
/// 1. The keyframes are of the target's type: `Matrix` for a transform, `Scalar` for an opacity
///    or a filter amount, and `Vector` for a scroll offset or a filter offset.
/// 2. There are at least two keyframes. Their offsets are non-decreasing, the first is 0 and the
///    last is 1.
/// 3. Every number is finite, except `Timing::iterations`, which may be infinite. The iterations
///    are not negative.
/// 4. Every easing is well formed, as [`easing_valid`] decides.
pub fn animation_valid(animation: &Animation) -> bool {
    let typed = match (&animation.target, &animation.keyframes) {
        (AnimationTarget::Transform(_), Keyframes::Matrix(_))
        | (AnimationTarget::Opacity(_), Keyframes::Scalar(_))
        | (AnimationTarget::ScrollOffset(_), Keyframes::Vector(_)) => true,
        (AnimationTarget::Filter { param, .. }, Keyframes::Scalar(_)) => {
            *param == FilterParam::Amount
        }
        (AnimationTarget::Filter { param, .. }, Keyframes::Vector(_)) => {
            *param == FilterParam::Offset
        }
        _ => false,
    };

    let frames = match &animation.keyframes {
        Keyframes::Matrix(frames) => frames_valid(frames, |matrix| {
            matrix
                .columns
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        }),
        Keyframes::Scalar(frames) => frames_valid(frames, |value| value.is_finite()),
        Keyframes::Vector(frames) => {
            frames_valid(frames, |value| value.iter().all(|value| value.is_finite()))
        }
    };

    typed && frames && timing_valid(&animation.timing)
}

/// Reports whether a timing is well formed: its iterations are not negative or NaN, and its
/// easing is well formed.
pub fn timing_valid(timing: &Timing) -> bool {
    timing.iterations >= 0.0 && easing_valid(&timing.easing)
}

/// Reports whether an easing is well formed:
///
/// - A cubic Bézier has finite control points.
/// - A step easing has at least one step, and at least two with `StepJump::None`.
/// - A piecewise-linear easing has at least one point, every number finite and its inputs
///   non-decreasing.
/// - A spring has a positive mass and stiffness, a damping that is not negative, and a finite
///   velocity.
pub fn easing_valid(easing: &Easing) -> bool {
    match easing {
        Easing::Linear => true,
        Easing::CubicBezier(points) => points.iter().all(|value| value.is_finite()),
        Easing::Steps { count, jump } => match jump {
            StepJump::None => *count >= 2,
            StepJump::Start | StepJump::End | StepJump::Both => *count >= 1,
        },
        Easing::LinearPoints(points) => {
            !points.is_empty()
                && points.iter().flatten().all(|value| value.is_finite())
                && points.windows(2).all(|pair| pair[0][0] <= pair[1][0])
        }
        Easing::Spring {
            mass,
            stiffness,
            damping,
            velocity,
        } => {
            mass.is_finite()
                && *mass > 0.0
                && stiffness.is_finite()
                && *stiffness > 0.0
                && damping.is_finite()
                && *damping >= 0.0
                && velocity.is_finite()
        }
    }
}

/// Reports whether keyframes are at least two, with valid offsets, values and easings.
fn frames_valid<T>(frames: &[Keyframe<T>], value_valid: impl Fn(&T) -> bool) -> bool {
    let (Some(first), Some(last)) = (frames.first(), frames.last()) else {
        return false;
    };
    frames.len() >= 2
        && first.offset == 0.0
        && last.offset == 1.0
        && frames
            .windows(2)
            .all(|pair| pair[0].offset <= pair[1].offset)
        && frames
            .iter()
            .all(|frame| value_valid(&frame.value) && easing_valid(&frame.easing))
}
