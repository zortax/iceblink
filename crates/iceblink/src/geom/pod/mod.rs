//! Plain-old-data implementations for the geometry types.
//!
//! Every geometry type is `#[repr(C)]` or `#[repr(transparent)]`, implements [`bytemuck::Pod`],
//! and has no padding bytes, so a value can be copied into a GPU buffer as it is. The table in
//! [`mod@assert`] checks the size, alignment and field offsets of each type at compile time.
//!
//! ```
//! use iceblink::geom::{Logical, LogicalPx, Point, Rect, Size};
//!
//! let rect: Rect<LogicalPx, Logical> = Rect::new(
//!     Point::new(LogicalPx(1.0), LogicalPx(2.0)),
//!     Size::new(LogicalPx(3.0), LogicalPx(4.0)),
//! );
//! assert_eq!(bytemuck::bytes_of(&rect).len(), 16);
//! ```

// The `Pod` and `Zeroable` impls below are the only unsafe code in the crate. The table in
// `assert` checks the layout each one relies on at compile time.
#![allow(unsafe_code)]

pub mod assert;

use bytemuck::{Pod, Zeroable};

use crate::geom::corners::{Corners, Vec2};
use crate::geom::edges::Edges;
use crate::geom::point::Point;
use crate::geom::rect::Rect;
use crate::geom::size::Size;
use crate::geom::space::Space;
use crate::geom::transform::{Affine2, Decomposed, Matrix4};
use crate::geom::unit::{DevicePx, LogicalPx, Scale};

// SAFETY: a `#[repr(transparent)]` wrapper has exactly the layout, size and validity of the type
// it wraps, and `f32` is itself plain old data with no invalid bit patterns.
unsafe impl Zeroable for LogicalPx {}
// SAFETY: as above.
unsafe impl Pod for LogicalPx {}
// SAFETY: as above.
unsafe impl Zeroable for DevicePx {}
// SAFETY: as above.
unsafe impl Pod for DevicePx {}

// SAFETY: `Scale` is `#[repr(transparent)]` over an `f32`. The space markers appear only inside a
// `PhantomData`, which occupies no bytes, so the layout is exactly that of an `f32`.
unsafe impl<Src: Space, Dst: Space> Zeroable for Scale<Src, Dst> {}
// SAFETY: as above.
unsafe impl<Src: Space, Dst: Space> Pod for Scale<Src, Dst> {}

// SAFETY: `#[repr(C)]` with two fields of the same plain-old-data type, so they are adjacent and
// the struct's size is exactly twice the field size, which leaves no room for padding. The
// space marker is a `PhantomData` and occupies no bytes.
unsafe impl<T: Zeroable, S: Space> Zeroable for Point<T, S> {}
// SAFETY: as above, and every bit pattern of the fields is valid because `T: Pod`.
unsafe impl<T: Pod, S: Space> Pod for Point<T, S> {}

// SAFETY: as for `Point`.
unsafe impl<T: Zeroable, S: Space> Zeroable for Size<T, S> {}
// SAFETY: as for `Point`.
unsafe impl<T: Pod, S: Space> Pod for Size<T, S> {}

// SAFETY: `#[repr(C)]` with a `Point<T, S>` followed by a `Size<T, S>`. Both have the same size
// and alignment and neither contains padding, so neither does the pair.
unsafe impl<T: Zeroable, S: Space> Zeroable for Rect<T, S> {}
// SAFETY: as above, and every bit pattern of the fields is valid because `T: Pod`.
unsafe impl<T: Pod, S: Space> Pod for Rect<T, S> {}

// SAFETY: `#[repr(C)]` with two fields of the same plain-old-data type.
unsafe impl<T: Zeroable> Zeroable for Vec2<T> {}
// SAFETY: as above, and every bit pattern of the fields is valid because `T: Pod`.
unsafe impl<T: Pod> Pod for Vec2<T> {}

// SAFETY: `#[repr(C)]` with four fields of the same plain-old-data type.
unsafe impl<T: Zeroable> Zeroable for Corners<T> {}
// SAFETY: as above, and every bit pattern of the fields is valid because `T: Pod`.
unsafe impl<T: Pod> Pod for Corners<T> {}

// SAFETY: `#[repr(C)]` with four fields of the same plain-old-data type.
unsafe impl<T: Zeroable> Zeroable for Edges<T> {}
// SAFETY: as above, and every bit pattern of the fields is valid because `T: Pod`.
unsafe impl<T: Pod> Pod for Edges<T> {}

// SAFETY: `#[repr(C)]` with six `f32` fields, which are equally sized and aligned, so the struct
// is exactly six floats with no padding and every bit pattern is a valid value.
unsafe impl Zeroable for Affine2 {}
// SAFETY: as above.
unsafe impl Pod for Affine2 {}

// SAFETY: `#[repr(C)]` around a single `[[f32; 4]; 4]`, which is sixteen contiguous floats.
unsafe impl Zeroable for Matrix4 {}
// SAFETY: as above.
unsafe impl Pod for Matrix4 {}

// SAFETY: `#[repr(C)]` with five `f32` arrays. Every field has alignment four and a size that is a
// multiple of four, so the fields are contiguous and the struct has no trailing padding.
unsafe impl Zeroable for Decomposed {}
// SAFETY: as above.
unsafe impl Pod for Decomposed {}
