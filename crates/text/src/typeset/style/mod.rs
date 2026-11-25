//! The styles a text is laid out with.

mod ink;
mod segments;

use iceblink::{Color, LineStyle, PaintId, PaintKind};
use parley::FontStyle;

pub(crate) use self::ink::Ink;
pub(crate) use self::segments::Segments;

/// A style that applies to the whole text or to a span of it.
///
/// A property set on a span replaces the one set for the whole text, and a property set again
/// on an overlapping span replaces it there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Property<'a> {
    /// The font families to look a font up in, as a CSS `font-family` list.
    Family(&'a str),
    /// The font size, in layout units.
    Size(f32),
    /// The font weight, from 1 to 1000, with 400 normal and 700 bold.
    Weight(f32),
    /// The slant of the font.
    Style(FontStyle),
    /// Values of variation axes, by axis tag such as `*b"wght"`.
    Variations(&'a [([u8; 4], f32)]),
    /// Extra space after every letter, in layout units.
    LetterSpacing(f32),
    /// The fill of the glyphs.
    Paint(TextPaint),
    /// Whether a line is drawn under the text.
    Underline(bool),
    /// Whether a line is drawn over the text.
    Overline(bool),
    /// Whether a line is drawn through the text.
    LineThrough(bool),
    /// The style of the decoration lines.
    DecorationStyle(LineStyle),
    /// The colour of the decoration lines, premultiplied.
    DecorationColor(Color),
}

/// The fill of glyphs or quads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextPaint {
    /// An inline colour, as `PaintRef::color` packs it: premultiplied gamma-encoded sRGB.
    Color([u8; 4]),
    /// A paint slot and the kind of the paint it holds.
    Slot(PaintKind, PaintId),
}

impl Default for TextPaint {
    /// Returns opaque black.
    fn default() -> Self {
        Self::Color([0, 0, 0, 255])
    }
}
