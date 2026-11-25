//! Paint slots, shader parameter slots and the references items hold to them.
//!
//! A slot is rewritten in place, and every item that refers to it is drawn with the new value
//! without being recorded again.

mod gradient;
mod params;
mod reference;

use crate::premultiplied::Color;

pub use self::gradient::{
    ColorSpace, Gradient, GradientShape, GradientStop, HueInterpolation, Interpolation,
};
pub use self::params::ParamsBlock;
pub use self::reference::{PaintKind, PaintRef};

/// The colour source of a paint slot.
///
/// Paint geometry is in the local space of the item that samples it, after the item's
/// `paint_origin` is subtracted where it has one.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    /// One colour everywhere.
    Solid(Color),
    /// A colour ramp.
    Gradient(Gradient),
    /// One colour per border side of a quad: top, right, bottom, left.
    ///
    /// Read anywhere other than a quad's border, it is its top colour.
    Sides([Color; 4]),
}

impl Paint {
    /// Returns the kind of a [`PaintRef`] to this paint.
    pub const fn kind(&self) -> PaintKind {
        match self {
            Self::Solid(_) => PaintKind::SOLID,
            Self::Gradient(_) => PaintKind::GRADIENT,
            Self::Sides(_) => PaintKind::SIDES,
        }
    }
}
