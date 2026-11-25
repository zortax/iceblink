//! The value of an animation at a progress: the keyframes around it and the interpolation
//! between them.

mod matrix;

use iceblink::{Keyframe, Keyframes, Matrix4};

use crate::motion::easing::ease;

pub use self::matrix::interpolate_matrix;

/// A value an animation applies, of the type of its keyframes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// A transform node's local matrix.
    Matrix(Matrix4),
    /// An opacity or a filter amount.
    Scalar(f32),
    /// A scroll offset or a drop-shadow offset.
    Vector([f32; 2]),
}

/// Returns the value of `keyframes` at eased iteration progress `progress`.
///
/// The progress selects a pair of neighbouring keyframes:
///
/// - Below 0, the first pair.
/// - From 1 on, the last pair.
/// - Otherwise the last keyframe at or before the progress and the next one.
///
/// The easing of the pair's first keyframe maps the progress within the pair, and the value is
/// interpolated between the two. Spring easings read `duration_s`, the iteration's length in
/// seconds.
///
/// Returns `None` for keyframes without a pair.
pub fn value_at(keyframes: &Keyframes, progress: f32, duration_s: f32) -> Option<Value> {
    match keyframes {
        Keyframes::Matrix(frames) => {
            let (from, to, share) = pair(frames, progress, duration_s)?;
            Some(Value::Matrix(interpolate_matrix(
                &from.value,
                &to.value,
                share,
            )))
        }
        Keyframes::Scalar(frames) => {
            let (from, to, share) = pair(frames, progress, duration_s)?;
            Some(Value::Scalar(lerp(from.value, to.value, share)))
        }
        Keyframes::Vector(frames) => {
            let (from, to, share) = pair(frames, progress, duration_s)?;
            Some(Value::Vector([
                lerp(from.value[0], to.value[0], share),
                lerp(from.value[1], to.value[1], share),
            ]))
        }
    }
}

/// Returns the keyframes around `progress` and the eased progress between them.
fn pair<T>(
    frames: &[Keyframe<T>],
    progress: f32,
    duration_s: f32,
) -> Option<(&Keyframe<T>, &Keyframe<T>, f32)> {
    if frames.len() < 2 {
        return None;
    }

    let last = frames.len() - 2;
    let at = if progress < 0.0 {
        0
    } else if progress >= 1.0 {
        last
    } else {
        frames
            .partition_point(|frame| frame.offset <= progress)
            .saturating_sub(1)
            .min(last)
    };

    let (from, to) = (&frames[at], &frames[at + 1]);
    let span = to.offset - from.offset;
    let share = if span > 0.0 {
        (progress - from.offset) / span
    } else if progress >= to.offset {
        1.0
    } else {
        0.0
    };
    Some((from, to, ease(&from.easing, share, duration_s)))
}

/// Returns the linear interpolation from `from` to `to` at `share`.
pub fn lerp(from: f32, to: f32, share: f32) -> f32 {
    from + (to - from) * share
}
