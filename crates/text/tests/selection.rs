//! Selection rectangles, carets and hit testing.

mod common;

use iceblink_text::typeset::{Affinity, Alignment, Layout, Position, Property};

use crate::common::typeset::{Fonts, SANS};

/// Two lines, "Wrapped " and "once here", broken at a soft wrap.
const TEXT: &str = "Wrapped once here";
const SECOND_LINE: usize = 8;

fn layout(fonts: &mut Fonts) -> Layout {
    let mut text = fonts.typesetter.text(TEXT);
    text.style(Property::Family(SANS))
        .style(Property::Size(20.0));
    let layout = text.build(Some(130.0), Alignment::Start);
    assert_eq!(
        layout.caret(Position::new(SECOND_LINE), 1.0)[0],
        0.0,
        "the second line starts at {SECOND_LINE}"
    );
    layout
}

/// Asserts that two rectangles or coordinates agree within float rounding.
fn assert_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{actual:?} against {expected:?}"
    );
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-3, "{actual:?} against {expected:?}");
    }
}

#[test]
fn a_selection_in_one_line_spans_the_carets_at_its_ends() {
    let mut fonts = Fonts::new();
    let layout = layout(&mut fonts);
    let start = layout.caret(Position::new(2), 0.0);
    let end = layout.caret(Position::new(6), 0.0);
    let rects = layout.selection(2..6);
    assert_eq!(rects.len(), 1);
    assert_close(
        &rects[0],
        &[start[0], start[1], end[0] - start[0], start[3]],
    );
    assert!(layout.selection(4..4).is_empty());
}

#[test]
fn a_selection_over_a_wrap_has_one_rectangle_per_line() {
    let mut fonts = Fonts::new();
    let layout = layout(&mut fonts);
    let rects = layout.selection(4..11);
    let first_end = layout.caret(
        Position {
            offset: SECOND_LINE,
            affinity: Affinity::Upstream,
        },
        0.0,
    );
    let second_end = layout.caret(Position::new(11), 0.0);
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert_close(
        &[rects[0][0] + rects[0][2], rects[0][1] + rects[0][3]],
        &[first_end[0], rects[1][1]],
    );
    assert_close(&[rects[1][0], rects[1][2]], &[0.0, second_end[0]]);
}

#[test]
fn a_caret_is_as_tall_as_its_line_and_as_wide_as_asked() {
    let mut fonts = Fonts::new();
    let layout = layout(&mut fonts);
    let first = layout.caret(Position::new(0), 2.0);
    let second = layout.caret(Position::new(SECOND_LINE + 1), 2.0);
    assert_close(&[first[0], first[1], first[2]], &[0.0, 0.0, 2.0]);
    assert_close(
        &[second[1], second[2], second[3]],
        &[first[3], 2.0, first[3]],
    );
}

#[test]
fn a_hit_gives_the_position_whose_caret_is_nearest() {
    let mut fonts = Fonts::new();
    let layout = layout(&mut fonts);
    for offset in [0, 1, 5, SECOND_LINE, SECOND_LINE + 3, TEXT.len()] {
        let caret = layout.caret(Position::new(offset), 0.0);
        let hit = layout.hit_test([caret[0] + 0.5, caret[1] + caret[3] / 2.0]);
        assert_eq!(hit.offset, offset);
    }
    let below = layout.hit_test([1.0, layout.size()[1] + 50.0]);
    assert_eq!(below.offset, SECOND_LINE);
}
