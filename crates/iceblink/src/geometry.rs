//! Rectangles and matrices shared by items, nodes and frames.

pub use crate::geom::Matrix4;

/// A rectangle in local space, as `[x, y, width, height]`.
///
/// A rectangle whose width or height is zero or negative is empty and paints nothing.
pub type LocalRect = [f32; 4];

/// A rectangle of whole device pixels.
///
/// It covers the pixels `origin.x .. origin.x + size.width` and
/// `origin.y .. origin.y + size.height`.
pub type DeviceRect = crate::geom::Rect<i32, crate::geom::Device>;
