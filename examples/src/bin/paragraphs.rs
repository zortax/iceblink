//! Paragraphs laid out by typeset in several fonts and weights, with decorations, a selection and
//! a blinking caret, wrapped to the window.

use std::ops::Range;
use std::sync::Arc;

use iceblink::FontRegistry as _;
use iceblink::{ChunkKey, Color, LineStyle, PaintId, PaintKind, Transaction, TransformId};
use iceblink_examples::{Demo, Scene, quad, solid};
use iceblink_testing::fonts::{LATIN, VARIABLE, read_font};
use iceblink_text::typeset::{
    Alignment, FontRegistry, FontStyle, Layout, Position, Property, TextPaint, TextRecorder,
    Typesetter,
};

const BACKGROUND: ChunkKey = ChunkKey::new(1, 0);
const TEXT: ChunkKey = ChunkKey::new(2, 0);
const CARET: ChunkKey = ChunkKey::new(3, 0);

/// The distance of the text from the window's edges.
const MARGIN: f32 = 32.0;

/// The gap between paragraphs.
const GAP: f32 = 20.0;

/// How long the caret stays shown, and then hidden, in seconds.
const BLINK: f32 = 0.5;

const SANS: &str = "Noto Sans";
const ROUNDED: &str = "Rubik";

const TITLE: &str = "Text through the compositor";
const BODY: &str = "Typeset lays text out over Parley and records it as glyph runs, decoration \
    lines and quads. This paragraph mixes bold, italic and semibold words, underlines, overlines \
    and struck-through text, wavy and dotted lines, wide letter spacing and an inline red. It \
    wraps to the window and is justified.";
const WEIGHTS: &str = "Hollo moon Hollo moon Hollo moon Hollo moon";
const SYSTEM: &str = "Serif from the system fonts, and monospace beside it.";
const CENTRED: &str = "Centred lines in a slanted face, broken at the width of the window.";

/// The selected words of the body. The caret sits at their end.
const SELECTED: &str = "records it as glyph runs, decoration lines and quads";

struct Paragraphs {
    space: TransformId,
    paints: Paints,
    paragraphs: Vec<Paragraph>,
    selection: Range<usize>,
    caret: Position,
    /// Whether the caret was last recorded shown.
    shown: bool,
}

struct Paints {
    background: PaintId,
    ink: PaintId,
    highlight: PaintId,
}

struct Paragraph {
    layout: Layout,
    alignment: Alignment,
    origin: [f32; 2],
}

impl Demo for Paragraphs {
    const TITLE: &'static str = "iceblink paragraphs";

    fn fonts() -> Option<Arc<FontRegistry>> {
        let registry = Arc::new(FontRegistry::new());
        registry.register(read_font(LATIN));
        registry.register(read_font(VARIABLE));
        Some(registry)
    }

    fn new(scene: &mut Scene, tx: &mut Transaction, size: [f32; 2]) -> Self {
        let registry = Arc::clone(scene.fonts().expect("the demo's fonts are shared"));

        let paints = Paints {
            background: solid(tx, [0.97, 0.96, 0.93, 1.0]),
            ink: solid(tx, [0.12, 0.13, 0.17, 1.0]),
            highlight: solid(tx, [0.72, 0.82, 1.0, 1.0]),
        };
        let mut typesetter = Typesetter::new(registry);
        let ink = TextPaint::Slot(PaintKind::SOLID, paints.ink);
        let paragraphs = vec![
            title(&mut typesetter, ink),
            body(&mut typesetter, ink),
            weights(&mut typesetter, ink),
            system(&mut typesetter, ink),
            centred(&mut typesetter, ink),
        ];
        let selection = range(BODY, SELECTED);
        let mut demo = Self {
            space: scene.space(),
            paints,
            paragraphs,
            caret: Position::new(selection.end),
            selection,
            shown: true,
        };

        demo.record(scene, tx, size);
        scene.append(tx, BACKGROUND);
        scene.append(tx, TEXT);
        scene.append(tx, CARET);
        demo
    }

    fn resize(&mut self, scene: &Scene, tx: &mut Transaction, size: [f32; 2]) {
        self.record(scene, tx, size);
    }

    fn advance(&mut self, tx: &mut Transaction, seconds: f32) -> bool {
        let shown = ((seconds / BLINK) as u32).is_multiple_of(2);
        if shown != self.shown {
            self.shown = shown;
            self.record_caret(tx);
        }
        true
    }
}

impl Paragraphs {
    /// Breaks every paragraph at the width of an output of `size`, stacks them, and records the
    /// background, the text and the caret.
    fn record(&mut self, scene: &Scene, tx: &mut Transaction, size: [f32; 2]) {
        let width = (size[0] - 2.0 * MARGIN).max(80.0);
        let mut y = MARGIN;
        for paragraph in &mut self.paragraphs {
            paragraph
                .layout
                .break_lines(Some(width), paragraph.alignment);
            paragraph.origin = [MARGIN, y];
            y += paragraph.layout.size()[1] + GAP;
        }

        let mut chunk = tx.record(BACKGROUND, Scene::props(scene.space()));
        let fill = chunk.paint_ref(PaintKind::SOLID, self.paints.background);
        chunk.quad(quad([0.0, 0.0, size[0], size[1]], fill));

        let mut chunk = tx.record(TEXT, Scene::props(scene.space()));
        let mut recorder = TextRecorder::new(&mut chunk);
        let body = &self.paragraphs[1];
        let highlight = TextPaint::Slot(PaintKind::SOLID, self.paints.highlight);
        recorder.selection(&body.layout, self.selection.clone(), body.origin, highlight);
        for paragraph in &self.paragraphs {
            recorder.layout(&paragraph.layout, paragraph.origin);
        }

        self.record_caret(tx);
    }

    /// Records the caret chunk, holding the caret while it is shown and nothing otherwise.
    fn record_caret(&self, tx: &mut Transaction) {
        let mut chunk = tx.record(CARET, Scene::props(self.space));
        if self.shown {
            let body = &self.paragraphs[1];
            let ink = TextPaint::Slot(PaintKind::SOLID, self.paints.ink);
            TextRecorder::new(&mut chunk).caret(&body.layout, self.caret, 2.0, body.origin, ink);
        }
    }
}

fn title(typesetter: &mut Typesetter, ink: TextPaint) -> Paragraph {
    let mut text = typesetter.text(TITLE);
    text.style(Property::Family(SANS))
        .style(Property::Size(30.0))
        .style(Property::Weight(700.0))
        .style(Property::Paint(ink));
    paragraph(text.build(None, Alignment::Start), Alignment::Start)
}

fn body(typesetter: &mut Typesetter, ink: TextPaint) -> Paragraph {
    let wavy = Color([0.85, 0.3, 0.1, 1.0]);
    let mut text = typesetter.text(BODY);
    text.style(Property::Family(SANS))
        .style(Property::Size(18.0))
        .style(Property::Paint(ink))
        .span(range(BODY, "bold"), Property::Weight(700.0))
        .span(range(BODY, "italic"), Property::Style(FontStyle::Italic))
        .span(range(BODY, "semibold"), Property::Weight(600.0))
        .span(range(BODY, "underlines"), Property::Underline(true))
        .span(range(BODY, "overlines"), Property::Overline(true))
        .span(range(BODY, "struck-through"), Property::LineThrough(true))
        .span(range(BODY, "wavy"), Property::Underline(true))
        .span(
            range(BODY, "wavy"),
            Property::DecorationStyle(LineStyle::WAVY),
        )
        .span(range(BODY, "wavy"), Property::DecorationColor(wavy))
        .span(range(BODY, "dotted"), Property::Underline(true))
        .span(
            range(BODY, "dotted"),
            Property::DecorationStyle(LineStyle::DOTTED),
        )
        .span(
            range(BODY, "wide letter spacing"),
            Property::LetterSpacing(3.0),
        )
        .span(
            range(BODY, "an inline red"),
            Property::Paint(TextPaint::Color([200, 30, 30, 255])),
        );
    paragraph(text.build(None, Alignment::Justify), Alignment::Justify)
}

/// The variable face at four weights along its weight axis.
fn weights(typesetter: &mut Typesetter, ink: TextPaint) -> Paragraph {
    let mut text = typesetter.text(WEIGHTS);
    text.style(Property::Family(ROUNDED))
        .style(Property::Size(30.0))
        .style(Property::Paint(ink));
    let words = WEIGHTS.len() / 4;
    for (index, weight) in [300.0, 500.0, 700.0, 900.0].into_iter().enumerate() {
        let start = index * (words + 1);
        text.span(start..start + words, Property::Weight(weight));
    }
    paragraph(text.build(None, Alignment::Start), Alignment::Start)
}

fn system(typesetter: &mut Typesetter, ink: TextPaint) -> Paragraph {
    let mut text = typesetter.text(SYSTEM);
    text.style(Property::Family("serif"))
        .style(Property::Size(20.0))
        .style(Property::Paint(ink))
        .span(
            range(SYSTEM, "monospace beside it."),
            Property::Family("monospace"),
        );
    paragraph(text.build(None, Alignment::Start), Alignment::Start)
}

fn centred(typesetter: &mut Typesetter, ink: TextPaint) -> Paragraph {
    let mut text = typesetter.text(CENTRED);
    text.style(Property::Family(SANS))
        .style(Property::Size(22.0))
        .style(Property::Style(FontStyle::Italic))
        .style(Property::Paint(ink));
    paragraph(text.build(None, Alignment::Center), Alignment::Center)
}

fn paragraph(layout: Layout, alignment: Alignment) -> Paragraph {
    Paragraph {
        layout,
        alignment,
        origin: [0.0, 0.0],
    }
}

/// Returns the byte range of the first `words` in `text`.
///
/// # Panics
///
/// Panics if `text` does not contain `words`.
fn range(text: &str, words: &str) -> Range<usize> {
    let start = text.find(words).expect("the text contains the words");
    start..start + words.len()
}

fn main() {
    iceblink_examples::run::<Paragraphs>();
}
