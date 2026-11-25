//! Underlines, overlines and lines through a glyph run.

use iceblink::{ClipRef, Line, LineStyle, SpaceRef};
use parley::GlyphRun;

use crate::typeset::style::Ink;

/// The decoration lines of a run.
pub(super) struct Decorations {
    /// The underline and the overline, drawn before the glyphs.
    pub(super) below: [Option<Line>; 2],
    /// The line through, drawn after the glyphs.
    pub(super) through: Option<Line>,
}

/// Returns the decoration lines `ink` asks for along `run`.
///
/// Each line spans the run's advance. A solid, dashed or dotted line is as tall as its
/// thickness, and a wavy or double line three times as tall, around the same middle.
pub(super) fn decorations(run: &GlyphRun<'_, Ink>, ink: &Ink) -> Decorations {
    let metrics = run.run().metrics();
    let baseline = run.baseline();
    let line = |on: bool, top: f32, thickness: f32| on.then(|| line(run, ink, top, thickness));

    let under = line(
        ink.underline,
        baseline - metrics.underline_offset,
        metrics.underline_size,
    );
    let over = line(
        ink.overline,
        baseline - metrics.ascent,
        metrics.underline_size,
    );
    let through = line(
        ink.line_through,
        baseline - metrics.strikethrough_offset,
        metrics.strikethrough_size,
    );
    Decorations {
        below: [under, over],
        through,
    }
}

/// Returns a line along `run` whose stroke starts `top` down and is `thickness` thick.
fn line(run: &GlyphRun<'_, Ink>, ink: &Ink, top: f32, thickness: f32) -> Line {
    let height = match ink.style {
        LineStyle::WAVY | LineStyle::DOUBLE => 3.0 * thickness,
        _ => thickness,
    };
    let middle = top + thickness / 2.0;
    Line {
        bounds: [run.offset(), middle - height / 2.0, run.advance(), height],
        color: ink.line_color(),
        thickness,
        clip: ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
        style: ink.style,
        _pad: [0; 3],
    }
}
