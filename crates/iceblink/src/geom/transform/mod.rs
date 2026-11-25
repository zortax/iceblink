//! Two- and three-dimensional transforms.
//!
//! [`Affine2`] is a transform within the plane: translation, scale, rotation, skew and their
//! compositions, stored as six floats. [`Matrix4`] is a full 4x4 matrix with perspective, laid out
//! the way a shader receives a `mat4x4<f32>`. [`Decomposed`] is a [`Matrix4`] split into the
//! parts that are interpolated when animating between two transforms.

pub mod affine2;
pub mod bounds;
pub mod decompose;
pub mod matrix4;

pub use crate::geom::transform::affine2::Affine2;
pub use crate::geom::transform::bounds::{transformed_bounds, translated_bounds};
pub use crate::geom::transform::decompose::Decomposed;
pub use crate::geom::transform::matrix4::Matrix4;
