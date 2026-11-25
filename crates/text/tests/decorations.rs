//! Underlines, overlines and lines through text.

mod common;

use iceblink::{ClientId, Color, IdSpace, Line, LineStyle, PaintKind};
use iceblink_text::typeset::{Alignment, Layout, Property, TextItem, TextPaint};

use crate::common::typeset::{Fonts, SANS};

fn decorated(fonts: &mut Fonts, properties: &[Property<'_>]) -> Layout {
    let mut text = fonts.typesetter.text("plain decorated");
    text.style(Property::Family(SANS))
        .style(Property::Size(30.0));
    for &property in properties {
        text.span(6..15, property);
    }
    text.build(None, Alignment::Start)
}

/// Returns the items of `layout` after its first run, as lines and the baseline of each run.
fn after_first_run(layout: &Layout) -> Vec<Result<Line, f32>> {
    layout
        .items()
        .skip(1)
        .map(|item| match item {
            TextItem::Run(run) => Err(run.glyphs[0].y),
            TextItem::Line(line) => Ok(*line),
        })
        .collect()
}

#[test]
fn lines_under_and_over_a_run_precede_it_and_a_line_through_follows() {
    let mut fonts = Fonts::new();
    let layout = decorated(
        &mut fonts,
        &[
            Property::Underline(true),
            Property::Overline(true),
            Property::LineThrough(true),
        ],
    );
    let items = after_first_run(&layout);
    let [Ok(under), Ok(over), Err(baseline), Ok(through)] = items[..] else {
        panic!("{items:?}");
    };
    let decorated_x = match layout.items().nth(3) {
        Some(TextItem::Run(run)) => run.glyphs[0].x,
        other => panic!("{other:?}"),
    };

    assert!(under.bounds[1] > baseline, "{under:?} under {baseline}");
    assert!(
        over.bounds[1] + over.bounds[3] < baseline - 20.0,
        "{over:?}"
    );
    let middle = through.bounds[1] + through.bounds[3] / 2.0;
    assert!(middle < baseline && middle > over.bounds[1], "{through:?}");
    for line in [under, over, through] {
        assert_eq!(line.bounds[0], decorated_x);
        assert!(line.bounds[2] > 100.0, "{line:?}");
        assert_eq!(line.bounds[3], line.thickness);
    }
}

#[test]
fn wavy_and_double_lines_are_three_strokes_tall_around_the_same_middle() {
    let mut fonts = Fonts::new();
    let solid = decorated(&mut fonts, &[Property::Underline(true)]);
    let [Ok(solid), ..] = after_first_run(&solid)[..] else {
        panic!("no underline");
    };
    for style in [LineStyle::WAVY, LineStyle::DOUBLE] {
        let layout = decorated(
            &mut fonts,
            &[Property::Underline(true), Property::DecorationStyle(style)],
        );
        let [Ok(line), ..] = after_first_run(&layout)[..] else {
            panic!("no underline");
        };
        assert_eq!(line.style, style);
        assert_eq!(line.bounds[3], 3.0 * line.thickness);
        let middle = |line: &Line| line.bounds[1] + line.bounds[3] / 2.0;
        assert!((middle(&line) - middle(&solid)).abs() < 1e-4);
    }
}

#[test]
fn lines_take_the_decoration_colour_then_the_inline_colour_then_black() {
    let mut fonts = Fonts::new();
    let color_of = |fonts: &mut Fonts, properties: &[Property<'_>]| {
        let layout = decorated(fonts, properties);
        match after_first_run(&layout)[..] {
            [Ok(line), ..] => line.color,
            ref items => panic!("{items:?}"),
        }
    };
    let red = Property::Paint(TextPaint::Color([255, 0, 0, 255]));
    let slot = Property::Paint(TextPaint::Slot(
        PaintKind::SOLID,
        IdSpace::new(ClientId(1)).mint(),
    ));
    let blue = Color([0.0, 0.0, 1.0, 1.0]);

    assert_eq!(
        color_of(&mut fonts, &[Property::Underline(true), red]),
        Color([1.0, 0.0, 0.0, 1.0])
    );
    assert_eq!(
        color_of(&mut fonts, &[Property::Underline(true), slot]),
        Color([0.0, 0.0, 0.0, 1.0])
    );
    assert_eq!(
        color_of(
            &mut fonts,
            &[
                Property::Underline(true),
                red,
                Property::DecorationColor(blue)
            ]
        ),
        blue
    );
}
