//! The glyph runs lowering takes.

use iceblink::{Glyph, GlyphRun};

use crate::content::target::Target;

/// A glyph run with its glyphs, its variation coordinates and what its output allows.
#[derive(Clone, Copy, Debug)]
pub struct GlyphRunInput<'a> {
    /// The run. Its paint is copied unchanged into the lowered sprites.
    pub run: &'a GlyphRun,
    /// The run's glyphs, from the chunk's glyph arena.
    pub glyphs: &'a [Glyph],
    /// The run's variation coordinates, from the chunk's coordinate arena.
    pub coords: &'a [i16],
    /// Whether the output may draw per-channel coverage for this run.
    ///
    /// It covers the conditions outside the run: the renderer reports `subpixel_text`, the frame
    /// request sets a stripe order, the output is opaque, the paint is an opaque solid colour and
    /// no group isolates the run. Lowering checks the run's own `SUBPIXEL` flag and its route.
    pub subpixel: bool,
    /// The output's scale factor, which sets the largest size on the atlas route.
    pub output_scale: f32,
}

/// A glyph run and where it is drawn, for lowering several runs at once.
#[derive(Clone, Copy, Debug)]
pub struct RunJob<'a> {
    /// The run.
    pub input: GlyphRunInput<'a>,
    /// Where the run is drawn.
    pub target: Target,
}
