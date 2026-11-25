//! The field offsets of the Pod items, which follow declaration order.
//!
//! Sizes and alignments are checked at compile time inside the crate.

use core::mem::offset_of;

use iceblink::{
    ChunkKey, GlyphRun, Image, ItemOp, Line, Marks, PaintRef, ParamsBlock, Quad, ShaderQuad, Shadow,
};

#[test]
fn fields_are_laid_out_in_declaration_order() {
    assert_eq!(offset_of!(Quad, fill), 80);
    assert_eq!(offset_of!(Quad, clip), 104);
    assert_eq!(offset_of!(Quad, style), 112);
    assert_eq!(offset_of!(ShaderQuad, shader), 112);
    assert_eq!(offset_of!(Shadow, color), 112);
    assert_eq!(offset_of!(Shadow, inset), 140);
    assert_eq!(offset_of!(Line, style), 44);
    assert_eq!(offset_of!(GlyphRun, flags), 48);
    assert_eq!(offset_of!(GlyphRun, palette), 50);
    assert_eq!(offset_of!(Image, paint), 96);
    assert_eq!(offset_of!(Image, flags), 120);
    assert_eq!(offset_of!(Image, extend), 121);
    assert_eq!(offset_of!(Image, filter), 123);
    assert_eq!(offset_of!(Marks, flags), 92);
    assert_eq!(offset_of!(ParamsBlock, user), 16);
    assert_eq!(offset_of!(PaintRef, index), 4);
    assert_eq!(offset_of!(ItemOp, index), 4);
    assert_eq!(offset_of!(ChunkKey, part), 8);
}

#[test]
fn an_item_round_trips_through_its_bytes() {
    let mut quad: Quad = bytemuck::Zeroable::zeroed();
    quad.bounds = [1.0, 2.0, 3.0, 4.0];
    quad.border = [5.0; 4];

    let bytes = bytemuck::bytes_of(&quad).to_vec();
    let back: Quad = bytemuck::pod_read_unaligned(&bytes);

    assert_eq!(back, quad);
}
