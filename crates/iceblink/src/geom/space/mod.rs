//! The coordinate spaces geometry is measured in.
//!
//! A space is a zero-sized marker type carried as a type parameter by
//! [`Point`](crate::geom::Point), [`Size`](crate::geom::Size) and [`Rect`](crate::geom::Rect). A
//! value from one space does not type-check where another space is expected.
//!
//! This module defines two spaces:
//!
//! - [`Logical`] is a resolution-independent space measured in
//!   [`LogicalPx`](crate::geom::LogicalPx).
//! - [`Device`] is the output surface's pixel grid, measured in
//!   [`DevicePx`](crate::geom::DevicePx).
//!
//! A [`Scale<Logical, Device>`](crate::geom::Scale) converts between the two. Clients define
//! further spaces by implementing [`Space`]:
//!
//! ```
//! use iceblink::geom::{Point, Space};
//!
//! #[derive(Clone, Copy, Debug)]
//! enum Page {}
//!
//! impl Space for Page {
//!     const NAME: &'static str = "page";
//! }
//!
//! let origin: Point<f32, Page> = Point::new(0.0, 0.0);
//! assert!(format!("{origin:?}").contains("page"));
//! ```

pub mod device;
pub mod logical;

pub(crate) mod derive;

pub use crate::geom::space::device::Device;
pub use crate::geom::space::logical::Logical;

/// A coordinate space that geometry can be measured in.
///
/// A space is usually an uninhabited enum, so it exists only as a type argument.
pub trait Space: Copy + Send + Sync + 'static {
    /// The space's name, shown in `Debug` output.
    ///
    /// ```
    /// use iceblink::geom::{Logical, Space};
    ///
    /// assert_eq!(Logical::NAME, "logical");
    /// ```
    const NAME: &'static str;
}

/// Declares an uninhabited space marker with the given name and doc lines.
macro_rules! space {
    ($name:ident, $text:literal, $($doc:literal),+ $(,)?) => {
        $(#[doc = $doc])+
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {}

        impl $crate::geom::space::Space for $name {
            const NAME: &'static str = $text;
        }
    };
}

pub(crate) use space;
