//! Items and paints with the defaults the demos share.

use iceblink::{
    BorderStyle, ClipRef, Color, PaintId, PaintRef, Quad, ROUND, SpaceRef, Transaction,
};

/// Returns a borderless quad over `bounds` filled with `fill`, in the chunk's space and clip.
pub fn quad(bounds: [f32; 4], fill: PaintRef) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [bounds[0], bounds[1]],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}

/// Creates a solid paint of the gamma-encoded sRGB colour `rgba` with straight alpha.
pub fn solid(tx: &mut Transaction, rgba: [f32; 4]) -> PaintId {
    let [r, g, b, a] = rgba;
    tx.create_paint(iceblink::Paint::Solid(Color::from_straight(r, g, b, a)))
}
