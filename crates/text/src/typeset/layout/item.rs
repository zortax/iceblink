//! The glyph runs and decoration lines a layout is drawn with.

use std::ops::Range;

use iceblink::{FontId, Glyph, Line};

use crate::typeset::layout::Layout;
use crate::typeset::style::TextPaint;

/// A glyph run or a decoration line of a layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextItem<'a> {
    /// Glyphs of one font at one size, with one paint.
    Run(TextRun<'a>),
    /// An underline, overline or line through text.
    Line(&'a Line),
}

/// Glyphs of one font at one size, with one paint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRun<'a> {
    /// The font's id in the layout's registry.
    pub font: FontId,
    /// The em size, in layout units.
    pub size: f32,
    /// The glyphs, with their origins on the baseline.
    pub glyphs: &'a [Glyph],
    /// Normalised variation coordinates in F2Dot14, one per axis of the font, or none for the
    /// default instance.
    pub coords: &'a [i16],
    /// The fill of the glyphs.
    pub paint: TextPaint,
    /// Synthetic emboldening for a weight the face lacks, in the units of
    /// [`GlyphRun::embolden`](iceblink::GlyphRun::embolden) at a scale of one.
    ///
    /// The value is zero for a weight the face has.
    pub embolden: f32,
    /// Synthetic slant for a style the face lacks, in degrees, positive leaning forwards.
    pub skew: f32,
}

/// An item of a layout, with its glyphs and coordinates as ranges of the layout's arrays.
pub(super) enum Piece {
    Run(RunPiece),
    Line(Line),
}

pub(super) struct RunPiece {
    pub(super) font: FontId,
    pub(super) size: f32,
    pub(super) glyphs: Range<usize>,
    pub(super) coords: Range<usize>,
    pub(super) paint: TextPaint,
    pub(super) embolden: f32,
    pub(super) skew: f32,
}

impl Piece {
    /// Returns the item, with its ranges resolved against the arrays of `layout`.
    pub(super) fn item<'a>(&'a self, layout: &'a Layout) -> TextItem<'a> {
        match self {
            Self::Run(run) => TextItem::Run(TextRun {
                font: run.font,
                size: run.size,
                glyphs: &layout.glyphs[run.glyphs.clone()],
                coords: &layout.coords[run.coords.clone()],
                paint: run.paint,
                embolden: run.embolden,
                skew: run.skew,
            }),
            Self::Line(line) => TextItem::Line(line),
        }
    }
}
