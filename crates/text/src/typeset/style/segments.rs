//! Ink properties set on spans, resolved into the ink of each stretch of text.

use std::ops::Range;

use crate::typeset::style::{Ink, Property};

/// Ink properties in the order they were set, over the default ink of the text.
#[derive(Default)]
pub(crate) struct Segments {
    base: Ink,
    spans: Vec<(Range<usize>, Property<'static>)>,
}

impl Segments {
    /// Applies the ink property `property` to the whole text.
    pub(crate) fn set_default(&mut self, property: &Property<'_>) {
        self.base.apply(property);
    }

    /// Adds the ink property `property` over `range`.
    pub(crate) fn set(&mut self, range: Range<usize>, property: &Property<'_>) {
        if let Some(property) = owned(property) {
            self.spans.push((range, property));
        }
    }

    pub(crate) fn base(&self) -> Ink {
        self.base
    }

    /// Returns the stretches of a text `len` bytes long whose ink differs from the default, in
    /// text order.
    ///
    /// Each stretch has one ink, which applies the spans covering it in the order they were set.
    pub(crate) fn resolve(&self, len: usize) -> Vec<(Range<usize>, Ink)> {
        let mut edges: Vec<usize> = self
            .spans
            .iter()
            .flat_map(|(range, _)| [range.start.min(len), range.end.min(len)])
            .collect();
        edges.sort_unstable();
        edges.dedup();

        let mut stretches: Vec<(Range<usize>, Ink)> = Vec::new();
        for pair in edges.windows(2) {
            let stretch = pair[0]..pair[1];
            let mut ink = self.base;
            for (range, property) in &self.spans {
                if range.start <= stretch.start && stretch.end <= range.end {
                    ink.apply(property);
                }
            }
            if ink == self.base {
                continue;
            }
            match stretches.last_mut() {
                Some((last, last_ink)) if last.end == stretch.start && *last_ink == ink => {
                    last.end = stretch.end;
                }
                _ => stretches.push((stretch, ink)),
            }
        }
        stretches
    }
}

/// Returns `property` without its borrow if it is an ink property.
fn owned(property: &Property<'_>) -> Option<Property<'static>> {
    match *property {
        Property::Paint(paint) => Some(Property::Paint(paint)),
        Property::Underline(on) => Some(Property::Underline(on)),
        Property::Overline(on) => Some(Property::Overline(on)),
        Property::LineThrough(on) => Some(Property::LineThrough(on)),
        Property::DecorationStyle(style) => Some(Property::DecorationStyle(style)),
        Property::DecorationColor(color) => Some(Property::DecorationColor(color)),
        _ => None,
    }
}
