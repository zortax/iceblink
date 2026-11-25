//! References from items to paint slots.

use bytemuck::{Pod, Zeroable};

use crate::bits::enumeration;

/// A paint slot an item is drawn with, together with the kind of its paint.
///
/// The kind matches the paint the slot holds. A reference of kind [`PaintKind::NONE`] paints
/// nothing, and its index is ignored.
///
/// A reference of kind [`PaintKind::INLINE`] refers to no slot. Its index holds a premultiplied
/// gamma-encoded sRGB colour as `u32::from_le_bytes([r, g, b, a])`, and it paints what a
/// [`Paint::Solid`](crate::Paint::Solid) slot of the colour `[r, g, b, a] / 255` paints. Frames
/// never hold such a reference.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct PaintRef {
    /// The kind of the paint in the slot.
    pub kind: PaintKind,
    /// Zero.
    pub _pad: [u8; 3],
    /// The slot, or the packed colour of an inline reference.
    pub index: u32,
}

impl PaintRef {
    /// A reference that paints nothing.
    pub const NONE: Self = Self {
        kind: PaintKind::NONE,
        _pad: [0; 3],
        index: 0,
    };

    /// Returns a reference that paints `rgba`, a premultiplied gamma-encoded sRGB colour, without
    /// a slot.
    pub const fn color(rgba: [u8; 4]) -> Self {
        Self {
            kind: PaintKind::INLINE,
            _pad: [0; 3],
            index: u32::from_le_bytes(rgba),
        }
    }

    /// Returns the colour of an inline reference, or `None` for every other kind.
    pub const fn inline_color(self) -> Option<[u8; 4]> {
        if self.kind.0 == PaintKind::INLINE.0 {
            Some(self.index.to_le_bytes())
        } else {
            None
        }
    }
}

enumeration! {
    /// The kind of paint a [`PaintRef`] refers to.
    pub struct PaintKind(u8) {
        /// No paint.
        NONE = 0,
        /// A [`Paint::Solid`](crate::Paint::Solid).
        SOLID = 1,
        /// A [`Paint::Gradient`](crate::Paint::Gradient).
        GRADIENT = 2,
        /// A [`Paint::Sides`](crate::Paint::Sides).
        SIDES = 4,
        /// A colour packed into the reference's index.
        INLINE = 8,
    }
}
