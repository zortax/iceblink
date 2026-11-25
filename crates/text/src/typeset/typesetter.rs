//! The layout context and the builder of a text's styles.

use std::ops::Range;
use std::sync::Arc;

use parley::{Alignment, FontContext, LayoutContext, RangedBuilder, StyleProperty};

use crate::typeset::layout::Layout;
use crate::typeset::registry::FontRegistry;
use crate::typeset::style::{Ink, Property, Segments};

/// A text layout engine over the fonts of a registry.
///
/// One typesetter serves many texts, and reuses its caches and scratch space between them.
pub struct Typesetter {
    registry: Arc<FontRegistry>,
    fonts: FontContext,
    layouts: LayoutContext<Ink>,
}

impl Typesetter {
    /// Returns a typesetter that looks fonts up in `registry`, including fonts registered later.
    pub fn new(registry: Arc<FontRegistry>) -> Self {
        let fonts = registry.font_context();
        Self {
            registry,
            fonts,
            layouts: LayoutContext::new(),
        }
    }

    /// Returns the registry fonts are looked up in.
    pub fn registry(&self) -> &Arc<FontRegistry> {
        &self.registry
    }

    /// Starts styling `text`.
    pub fn text<'a>(&'a mut self, text: &'a str) -> TextBuilder<'a> {
        let builder = self
            .layouts
            .ranged_builder(&mut self.fonts, text, 1.0, true);
        TextBuilder {
            registry: &self.registry,
            text,
            builder,
            segments: Segments::default(),
        }
    }
}

/// The styles of a text being built into a layout.
///
/// Ranges are byte ranges of the text.
pub struct TextBuilder<'a> {
    registry: &'a Arc<FontRegistry>,
    text: &'a str,
    builder: RangedBuilder<'a, Ink>,
    segments: Segments,
}

impl TextBuilder<'_> {
    /// Sets `property` for the whole text.
    pub fn style(&mut self, property: Property<'_>) -> &mut Self {
        let variations = variations(&property);
        match font_property(&property, &variations) {
            Some(font) => self.builder.push_default(font),
            None => self.segments.set_default(&property),
        }
        self
    }

    /// Sets `property` for the bytes in `range`.
    pub fn span(&mut self, range: Range<usize>, property: Property<'_>) -> &mut Self {
        let variations = variations(&property);
        match font_property(&property, &variations) {
            Some(font) => self.builder.push(font, range),
            None => self.segments.set(range, &property),
        }
        self
    }

    /// Shapes the text and breaks it into lines.
    ///
    /// Lines break at `width` if given, and are aligned within it, or within the widest line
    /// without one.
    pub fn build(mut self, width: Option<f32>, alignment: Alignment) -> Layout {
        self.builder
            .push_default(StyleProperty::Brush(self.segments.base()));
        for (range, ink) in self.segments.resolve(self.text.len()) {
            self.builder.push(StyleProperty::Brush(ink), range);
        }
        let inner = self.builder.build(self.text);
        Layout::new(inner, Arc::clone(self.registry), width, alignment)
    }
}

/// Returns the variation settings of a `Variations` property, and nothing for any other.
fn variations(property: &Property<'_>) -> Vec<parley::FontVariation> {
    match property {
        Property::Variations(values) => values
            .iter()
            .map(|(tag, value)| parley::FontVariation::new(parley::setting::Tag::new(tag), *value))
            .collect(),
        _ => Vec::new(),
    }
}

/// Converts a font property into parley's, with `variations` standing in for the values of a
/// `Variations` property, or returns `None` for an ink property.
fn font_property<'a>(
    property: &Property<'a>,
    variations: &'a [parley::FontVariation],
) -> Option<StyleProperty<'a, Ink>> {
    let font = match *property {
        Property::Family(families) => StyleProperty::FontFamily(parley::FontFamily::Source(
            std::borrow::Cow::Borrowed(families),
        )),
        Property::Size(size) => StyleProperty::FontSize(size),
        Property::Weight(weight) => StyleProperty::FontWeight(parley::FontWeight::new(weight)),
        Property::Style(style) => StyleProperty::FontStyle(style),
        Property::Variations(_) => StyleProperty::FontVariations(variations.into()),
        Property::LetterSpacing(spacing) => StyleProperty::LetterSpacing(spacing),
        Property::Paint(_)
        | Property::Underline(_)
        | Property::Overline(_)
        | Property::LineThrough(_)
        | Property::DecorationStyle(_)
        | Property::DecorationColor(_) => return None,
    };
    Some(font)
}
