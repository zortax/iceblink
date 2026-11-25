//! Variation coordinates and synthesis of glyph runs.

mod common;

use iceblink_testing::fonts::{VARIABLE, read_font};
use iceblink_text::typeset::{Alignment, FontStyle, Property, TextItem, TextRun};
use skrifa::MetadataProvider as _;

use crate::common::typeset::{Fonts, SANS, VARIABLE_FAMILY};

/// Returns the first glyph run of a short text in `family`, styled with `properties`.
fn first_run(fonts: &mut Fonts, family: &str, properties: &[Property<'_>]) -> RunFacts {
    let mut text = fonts.typesetter.text("lOWn");
    text.style(Property::Family(family))
        .style(Property::Size(24.0));
    for &property in properties {
        text.style(property);
    }
    let layout = text.build(None, Alignment::Start);
    let run = layout
        .items()
        .find_map(|item| match item {
            TextItem::Run(run) => Some(run),
            TextItem::Line(_) => None,
        })
        .expect("the text has a run");
    RunFacts::of(&run)
}

#[derive(Debug, PartialEq)]
struct RunFacts {
    coords: Vec<i16>,
    embolden: f32,
    skew: f32,
}

impl RunFacts {
    fn of(run: &TextRun<'_>) -> Self {
        Self {
            coords: run.coords.to_vec(),
            embolden: run.embolden,
            skew: run.skew,
        }
    }
}

/// Returns the normalised coordinates of the variable test face at weight `wght`.
fn coords_at(wght: f32) -> Vec<i16> {
    let data = read_font(VARIABLE);
    let font = skrifa::FontRef::new(data.bytes()).expect("the face parses");
    font.axes()
        .location([("wght", wght)])
        .coords()
        .iter()
        .map(|coord| coord.to_bits())
        .collect()
}

#[test]
fn variation_values_reach_the_run_as_normalised_coordinates() {
    let mut fonts = Fonts::new();
    let run = first_run(
        &mut fonts,
        VARIABLE_FAMILY,
        &[Property::Variations(&[(*b"wght", 650.0)])],
    );
    assert_eq!(run.coords, coords_at(650.0));
    assert_ne!(run.coords, coords_at(400.0));
}

#[test]
fn a_weight_the_face_has_an_axis_for_is_set_on_the_axis() {
    let mut fonts = Fonts::new();
    let run = first_run(&mut fonts, VARIABLE_FAMILY, &[Property::Weight(800.0)]);
    assert_eq!(
        run,
        RunFacts {
            coords: coords_at(800.0),
            embolden: 0.0,
            skew: 0.0,
        }
    );
}

#[test]
fn a_weight_and_slant_the_face_lacks_are_synthesised() {
    let mut fonts = Fonts::new();
    let run = first_run(
        &mut fonts,
        SANS,
        &[Property::Weight(700.0), Property::Style(FontStyle::Italic)],
    );
    assert!(run.coords.is_empty());
    assert!(run.embolden > 0.0, "{run:?}");
    assert!(run.skew > 0.0, "{run:?}");

    let regular = first_run(&mut fonts, SANS, &[]);
    assert_eq!(
        regular,
        RunFacts {
            coords: Vec::new(),
            embolden: 0.0,
            skew: 0.0,
        }
    );
}
