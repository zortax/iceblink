//! The arithmetic of compositor-driven motion: easings, the animation timing model, keyframe
//! interpolation and scroll physics.
//!
//! Every function is a pure function of its inputs, with time passed in, so a compositor samples
//! motion at whatever time its frames are predicted to reach the display.
//!
//! | Module | Contents |
//! | --- | --- |
//! | [`easing`] | The easing functions, from progress to eased progress |
//! | [`timing`] | The Web Animations timing model, from local time to iteration progress |
//! | [`value`] | Keyframe selection and interpolation of scalars, vectors and matrices |
//! | [`spring`] | A damped spring in closed form |
//! | [`physics`] | Fling decay, the overscroll bounce and the wheel step curve |
//! | [`check`] | Whether an animation's keyframes, timing and easings are well formed |
//!
//! ```
//! use iceblink_core::motion::easing::ease;
//! use iceblink::Easing;
//!
//! assert_eq!(ease(&Easing::Linear, 0.25, 1.0), 0.25);
//! ```

pub mod check;
pub mod easing;
pub mod physics;
pub mod spring;
pub mod timing;
pub mod value;
