//! Coordinate spaces, units, geometry, transforms and device-pixel snapping.
//!
//! Lengths are typed by unit. [`LogicalPx`] is a resolution-independent pixel and [`DevicePx`] is
//! a physical pixel on the output surface. Plain [`f32`] and [`i32`] serve as units too.
//!
//! [`Point`], [`Size`] and [`Rect`] carry a zero-sized marker for the space they were measured in,
//! such as [`Logical`] or [`Device`]. Mixing two spaces does not compile:
//!
//! ```compile_fail,E0277
//! # use iceblink::geom::{Device, Logical, LogicalPx, Point};
//! let logical: Point<LogicalPx, Logical> = Point::new(LogicalPx(1.0), LogicalPx(2.0));
//! let device: Point<LogicalPx, Device> = Point::new(LogicalPx(1.0), LogicalPx(2.0));
//! let _ = logical + device;
//! ```
//!
//! A [`Scale`] converts between two spaces and carries both in its type:
//!
//! ```
//! use iceblink::geom::{Device, DevicePx, Logical, LogicalPx, Point, Scale};
//!
//! let scale: Scale<Logical, Device> = Scale::new(2.0);
//! let logical: Point<LogicalPx, Logical> = Point::new(LogicalPx(3.0), LogicalPx(4.0));
//! let device: Point<DevicePx, Device> = logical * scale;
//! assert_eq!(device, Point::new(DevicePx(6.0), DevicePx(8.0)));
//! ```
//!
//! Every unit and geometry type is `#[repr(C)]` or `#[repr(transparent)]`, has no padding and
//! implements [`bytemuck::Pod`], so it can be copied into a GPU buffer as it is.
//!
//! | Module | Contents |
//! |---|---|
//! | [`mod@unit`] | [`LogicalPx`], [`DevicePx`], [`Scale`] and the [`Unit`] trait |
//! | [`space`] | The [`Space`] trait and the [`Logical`] and [`Device`] markers |
//! | [`point`], [`size`], [`rect`], [`edges`] | Space-tagged geometry and per-side lengths |
//! | [`corners`] | [`Corners`] and the elliptical radius pair [`Vec2`] |
//! | [`transform`] | [`Affine2`], [`Matrix4`], [`Decomposed`] and transformed bounds |
//! | [`snap`] | The device-pixel snapping rules |
//! | [`pod`] | The plain-old-data implementations and their layout checks |

pub mod corners;
pub mod edges;
pub mod pod;
pub mod point;
pub mod rect;
pub mod size;
pub mod snap;
pub mod space;
pub mod transform;
pub mod unit;

pub use crate::geom::corners::{Corners, Vec2};
pub use crate::geom::edges::Edges;
pub use crate::geom::point::Point;
pub use crate::geom::rect::Rect;
pub use crate::geom::size::Size;
pub use crate::geom::snap::{cover_bounds, snap_bounds, snap_edges, snap_stroke};
pub use crate::geom::space::{Device, Logical, Space};
pub use crate::geom::transform::{
    Affine2, Decomposed, Matrix4, transformed_bounds, translated_bounds,
};
pub use crate::geom::unit::{DevicePx, LogicalPx, Scale, Unit};
