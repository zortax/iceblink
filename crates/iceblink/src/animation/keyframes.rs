//! The values an animation passes through.

use smallvec::SmallVec;

use crate::animation::Easing;
use crate::geometry::Matrix4;

/// The values an animation passes through, of the type its target takes.
///
/// Offsets are non-decreasing, the first is 0 and the last is 1.
#[derive(Clone, Debug, PartialEq)]
pub enum Keyframes {
    /// Matrices, interpolated by decomposition.
    Matrix(SmallVec<[Keyframe<Matrix4>; 2]>),
    /// Numbers, interpolated linearly.
    Scalar(SmallVec<[Keyframe<f32>; 2]>),
    /// Pairs of numbers, interpolated linearly.
    Vector(SmallVec<[Keyframe<[f32; 2]>; 2]>),
}

/// One value of an animation.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe<T> {
    /// Where in an iteration the keyframe sits, from 0 to 1.
    pub offset: f32,
    /// The value at the offset.
    pub value: T,
    /// The easing from this keyframe to the next.
    pub easing: Easing,
}
