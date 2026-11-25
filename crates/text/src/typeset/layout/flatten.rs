//! Conversion of parley's lines into glyph runs and decoration lines.

use iceblink::Glyph;
use parley::PositionedLayoutItem;

use crate::typeset::layout::Layout;
use crate::typeset::layout::decoration::{Decorations, decorations};
use crate::typeset::layout::item::{Piece, RunPiece};

/// Synthetic emboldening as a fraction of the em size.
const EMBOLDEN: f32 = 0.02;

/// Rebuilds the runs and lines of `layout` from its parley lines.
pub(super) fn flatten(layout: &mut Layout) {
    layout.glyphs.clear();
    layout.coords.clear();
    layout.pieces.clear();

    for line in layout.inner.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            let ink = glyph_run.style().brush;
            let Decorations { below, through } = decorations(&glyph_run, &ink);

            let glyphs_start = layout.glyphs.len();
            layout
                .glyphs
                .extend(glyph_run.positioned_glyphs().map(|glyph| Glyph {
                    id: glyph.id,
                    x: glyph.x,
                    y: glyph.y,
                }));
            let coords_start = layout.coords.len();
            layout.coords.extend_from_slice(run.normalized_coords());

            let size = run.font_size();
            let synthesis = run.synthesis();
            layout
                .pieces
                .extend(below.into_iter().flatten().map(Piece::Line));
            layout.pieces.push(Piece::Run(RunPiece {
                font: layout.registry.id_of(run.font()),
                size,
                glyphs: glyphs_start..layout.glyphs.len(),
                coords: coords_start..layout.coords.len(),
                paint: ink.paint,
                embolden: if synthesis.embolden() {
                    EMBOLDEN * size
                } else {
                    0.0
                },
                skew: synthesis.skew().unwrap_or(0.0),
            }));
            layout.pieces.extend(through.map(Piece::Line));
        }
    }
}
