//! Recording layouts, selections and carets into a chunk.

mod paints;

use std::ops::Range;

use iceblink::{
    ArenaRange, BorderStyle, ChunkRecorder, ClipRef, Glyph, GlyphFlags, GlyphRun, LocalRect,
    PaintRef, Quad, ROUND, SpaceRef,
};

use crate::typeset::layout::{Layout, Position, TextItem, TextRun};
use crate::typeset::style::TextPaint;

use self::paints::Paints;

/// A recorder of layouts, selections and carets into a chunk.
///
/// Items are recorded in the chunk's space and clip. A paint slot gets one reference however
/// often the recorder uses it.
pub struct TextRecorder<'c, 'a> {
    chunk: &'c mut ChunkRecorder<'a>,
    paints: Paints,
    glyphs: Vec<Glyph>,
}

impl<'c, 'a> TextRecorder<'c, 'a> {
    /// Returns a recorder that records into `chunk`.
    pub fn new(chunk: &'c mut ChunkRecorder<'a>) -> Self {
        Self {
            chunk,
            paints: Paints::default(),
            glyphs: Vec::new(),
        }
    }

    /// Records the glyph runs and decoration lines of `layout`, with its top left at `origin`.
    pub fn layout(&mut self, layout: &Layout, origin: [f32; 2]) {
        for item in layout.items() {
            match item {
                TextItem::Run(run) => self.run(&run, origin),
                TextItem::Line(line) => {
                    let mut line = *line;
                    line.bounds = moved(line.bounds, origin);
                    self.chunk.line(line);
                }
            }
        }
    }

    /// Records the rectangles that cover the bytes in `range` of `layout` as quads filled with
    /// `fill`, with the layout's top left at `origin`.
    pub fn selection(
        &mut self,
        layout: &Layout,
        range: Range<usize>,
        origin: [f32; 2],
        fill: TextPaint,
    ) {
        let fill = self.paints.resolve(self.chunk, fill);
        for rect in layout.selection(range) {
            self.chunk.quad(quad(moved(rect, origin), fill));
        }
    }

    /// Records a caret `width` wide at `position` of `layout` as a quad filled with `fill`, with
    /// the layout's top left at `origin`.
    pub fn caret(
        &mut self,
        layout: &Layout,
        position: Position,
        width: f32,
        origin: [f32; 2],
        fill: TextPaint,
    ) {
        let fill = self.paints.resolve(self.chunk, fill);
        let rect = moved(layout.caret(position, width), origin);
        self.chunk.quad(quad(rect, fill));
    }

    fn run(&mut self, run: &TextRun<'_>, origin: [f32; 2]) {
        self.glyphs.clear();
        self.glyphs.extend(run.glyphs.iter().map(|glyph| Glyph {
            id: glyph.id,
            x: glyph.x + origin[0],
            y: glyph.y + origin[1],
        }));

        let glyphs = self.chunk.glyphs(&self.glyphs);
        let coords = if run.coords.is_empty() {
            ArenaRange::default()
        } else {
            self.chunk.coords(run.coords)
        };
        let paint = self.paints.resolve(self.chunk, run.paint);

        self.chunk.glyph_run(GlyphRun {
            font: run.font,
            size: run.size,
            glyphs,
            coords,
            paint,
            embolden: run.embolden,
            skew: run.skew,
            clip: ClipRef::INHERIT,
            space: SpaceRef::INHERIT,
            flags: GlyphFlags::default(),
            _pad: [0; 1],
            palette: 0,
        });
    }
}

fn moved(rect: LocalRect, origin: [f32; 2]) -> LocalRect {
    [rect[0] + origin[0], rect[1] + origin[1], rect[2], rect[3]]
}

/// Returns a square borderless quad over `bounds` filled with `fill`.
fn quad(bounds: LocalRect, fill: PaintRef) -> Quad {
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
