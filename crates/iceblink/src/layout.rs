//! Compile-time checks of the sizes and alignments the contract fixes for its Pod types.

use crate::{
    ArenaRange, Backdrop, ChunkKey, Color, ExternalQuad, FontId, Glyph, GlyphRun, Image, ItemOp,
    Line, MarkBox, MarkPlacement, Marks, PaintRef, ParamsBlock, Quad, ShaderQuad, Shadow,
};

/// Asserts at compile time that each type has the given size and alignment in bytes.
macro_rules! layout {
    ($($ty:ty => $size:expr, $align:expr;)+) => {
        $(
            const _: () = assert!(
                core::mem::size_of::<$ty>() == $size && core::mem::align_of::<$ty>() == $align
            );
        )+
    };
}

pub(crate) use layout;

layout! {
    Quad => 116, 4;
    ShaderQuad => 124, 4;
    Shadow => 144, 4;
    Line => 48, 4;
    GlyphRun => 52, 4;
    Image => 124, 4;
    Marks => 96, 4;
    ExternalQuad => 32, 4;
    Backdrop => 28, 4;
    PaintRef => 8, 4;
    Glyph => 12, 4;
    MarkBox => 64, 4;
    MarkPlacement => 16, 4;
    ParamsBlock => 80, 4;
    ItemOp => 8, 4;
    ArenaRange => 8, 4;
    Color => 16, 4;
    FontId => 4, 4;
    ChunkKey => 16, 8;
}
