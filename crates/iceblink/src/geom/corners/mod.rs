//! Per-corner values and elliptical corner radii.
//!
//! [`Corners<T>`] holds one `T` per corner of a rectangle. Corner radii are `Corners<Vec2<T>>`:
//! every corner has a horizontal and a vertical radius and is a quadrant of an ellipse.
//! [`Vec2::splat`] creates a circular corner.

pub mod elliptical;
pub mod radius;

pub use crate::geom::corners::elliptical::Vec2;
pub use crate::geom::corners::radius::Corners;
