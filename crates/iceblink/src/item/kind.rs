//! The record order of a chunk's items across kinds.

use bytemuck::{Pod, Zeroable};

use crate::bits::enumeration;

/// One item in record order: its kind, and its index in that kind's array.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct ItemOp {
    /// The item's kind, which selects the array it is in.
    pub kind: ItemKind,
    /// Zero.
    pub _pad: [u8; 3],
    /// The item's index in the array of its kind.
    pub index: u32,
}

enumeration! {
    /// The kind of a recorded item.
    pub struct ItemKind(u8) {
        /// A [`Quad`](crate::Quad).
        QUAD = 0,
        /// A [`ShaderQuad`](crate::ShaderQuad).
        SHADER_QUAD = 1,
        /// A [`Shadow`](crate::Shadow).
        SHADOW = 2,
        /// A [`Line`](crate::Line).
        LINE = 3,
        /// A [`GlyphRun`](crate::GlyphRun).
        GLYPH_RUN = 4,
        /// An [`Image`](crate::Image).
        IMAGE = 5,
        /// A [`Marks`](crate::Marks).
        MARKS = 6,
        /// A [`PathItem`](crate::PathItem).
        PATH = 7,
        /// An [`ExternalQuad`](crate::ExternalQuad).
        EXTERNAL = 8,
        /// A [`Backdrop`](crate::Backdrop).
        BACKDROP = 9,
    }
}
