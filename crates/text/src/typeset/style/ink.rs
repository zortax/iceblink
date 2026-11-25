//! The paint and decorations of a span, stored as parley's brush.

use iceblink::{Color, LineStyle};

use crate::typeset::style::{Property, TextPaint};

/// The paint and decorations of a span.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Ink {
    pub(crate) paint: TextPaint,
    pub(crate) underline: bool,
    pub(crate) overline: bool,
    pub(crate) line_through: bool,
    pub(crate) style: LineStyle,
    pub(crate) color: Option<Color>,
}

impl Ink {
    /// Applies `property` if it is a paint or decoration property.
    pub(crate) fn apply(&mut self, property: &Property<'_>) {
        match *property {
            Property::Paint(paint) => self.paint = paint,
            Property::Underline(on) => self.underline = on,
            Property::Overline(on) => self.overline = on,
            Property::LineThrough(on) => self.line_through = on,
            Property::DecorationStyle(style) => self.style = style,
            Property::DecorationColor(color) => self.color = Some(color),
            _ => {}
        }
    }

    /// Returns the colour decoration lines are drawn in.
    ///
    /// The colour falls back to the inline colour of the paint, and to opaque black for a paint
    /// slot.
    pub(crate) fn line_color(&self) -> Color {
        if let Some(color) = self.color {
            return color;
        }

        let rgba = match self.paint {
            TextPaint::Color(rgba) => rgba,
            TextPaint::Slot(..) => [0, 0, 0, 255],
        };
        Color::from_rgba8(rgba)
    }
}
