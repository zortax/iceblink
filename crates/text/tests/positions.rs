//! Glyph runs hold the glyphs and positions of Parley's own layout of the same text.

mod common;

use iceblink_testing::fonts::{LATIN, read_font};
use iceblink_text::typeset::{Alignment, Property, TextItem};
use parley::{
    AlignmentOptions, FontContext, FontFamily, FontWeight, LayoutContext, PositionedLayoutItem,
    StyleProperty,
};

use crate::common::typeset::{Fonts, SANS};

const TEXT: &str = "Typeset lays text out over Parley, wrapping it into centred lines.";
const WIDTH: f32 = 180.0;
const BOLD: std::ops::Range<usize> = 8..12;

/// Returns the glyph ids and positions of parley's layout of `TEXT`, in paint order.
fn parley_glyphs() -> Vec<(u32, f32, f32, f32)> {
    let mut fonts = FontContext {
        collection: fontique::Collection::new(fontique::CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: fontique::SourceCache::default(),
    };
    let file = read_font(LATIN).file;
    fonts
        .collection
        .register_fonts(fontique::Blob::new(file), None);

    let mut layouts = LayoutContext::<()>::new();
    let mut builder = layouts.ranged_builder(&mut fonts, TEXT, 1.0, true);
    builder.push_default(StyleProperty::FontFamily(FontFamily::named(SANS)));
    builder.push_default(StyleProperty::FontSize(17.0));
    builder.push_default(StyleProperty::LetterSpacing(0.5));
    builder.push(StyleProperty::FontWeight(FontWeight::BOLD), BOLD);
    let mut layout = builder.build(TEXT);
    layout.break_all_lines(Some(WIDTH));
    layout.align(Alignment::Center, AlignmentOptions::default());

    let mut glyphs = Vec::new();
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(run) = item else {
                continue;
            };
            let size = run.run().font_size();
            glyphs.extend(
                run.positioned_glyphs()
                    .map(|glyph| (glyph.id, glyph.x, glyph.y, size)),
            );
        }
    }
    glyphs
}

#[test]
fn runs_hold_the_glyphs_parley_places() {
    let mut fonts = Fonts::new();
    let mut text = fonts.typesetter.text(TEXT);
    text.style(Property::Family(SANS))
        .style(Property::Size(17.0))
        .style(Property::LetterSpacing(0.5))
        .span(BOLD, Property::Weight(700.0));
    let layout = text.build(Some(WIDTH), Alignment::Center);

    let ours: Vec<(u32, f32, f32, f32)> = layout
        .items()
        .filter_map(|item| match item {
            TextItem::Run(run) => Some(run),
            TextItem::Line(_) => None,
        })
        .flat_map(|run| {
            assert_eq!(run.font, fonts.latin);
            run.glyphs
                .iter()
                .map(move |glyph| (glyph.id, glyph.x, glyph.y, run.size))
        })
        .collect();
    let expected = parley_glyphs();
    assert!(
        expected.iter().map(|glyph| glyph.2).max_by(f32::total_cmp)
            > expected.iter().map(|glyph| glyph.2).min_by(f32::total_cmp),
        "the text wraps"
    );
    assert_eq!(ours, expected);
}
