//! The nodes and items the bench lists are made of.

use iceblink::{
    Anchor, ArenaRange, BorderStyle, ChunkFlags, ChunkProps, ClipId, ClipRef, FontId, GlyphFlags,
    GlyphRun, Matrix4, PaintRef, Quad, ROUND, ScrollId, SpaceRef, TransformId, TransformNode,
};

/// Returns a transform node with an identity matrix under `parent`, scrolled by `scroll`.
pub(crate) fn node(parent: Option<TransformId>, scroll: Option<ScrollId>) -> TransformNode {
    TransformNode {
        parent,
        local: Matrix4::IDENTITY,
        scroll,
        anchor: Anchor::Scrolls,
    }
}

/// Returns chunk props in `space` under `clip`.
pub(crate) fn props(space: TransformId, clip: Option<ClipId>) -> ChunkProps {
    ChunkProps {
        space,
        clip,
        flags: ChunkFlags::default(),
    }
}

/// Returns a borderless quad over `bounds` filled with `fill`.
pub(crate) fn quad(bounds: [f32; 4], fill: PaintRef) -> Quad {
    Quad {
        bounds,
        radii: [0.0; 8],
        shapes: ROUND,
        border: [0.0; 4],
        fill,
        stroke: PaintRef::NONE,
        paint_origin: [0.0; 2],
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: BorderStyle::SOLID,
        _pad: [0; 3],
    }
}

/// Returns a 12 pixel glyph run of `glyphs` in `font`, filled with `paint`.
pub(crate) fn run(font: FontId, glyphs: ArenaRange, paint: PaintRef) -> GlyphRun {
    GlyphRun {
        font,
        size: 12.0,
        glyphs,
        coords: ArenaRange::default(),
        paint,
        embolden: 0.0,
        skew: 0.0,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        flags: GlyphFlags::default(),
        _pad: [0; 1],
        palette: 0,
    }
}
