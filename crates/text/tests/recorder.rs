//! Layouts recorded into a chunk at an origin.

mod common;

use iceblink::Compositor as _;
use iceblink::{
    AnyId, ChunkFlags, ChunkKey, ChunkProps, Matrix4, PaintKind, RecordedChunk, TransformNode,
};
use iceblink_core::CompositorImpl;
use iceblink_text::typeset::{
    Alignment, Layout, Position, Property, TextItem, TextPaint, TextRecorder,
};

use crate::common::typeset::{Fonts, VARIABLE_FAMILY};

const ORIGIN: [f32; 2] = [30.0, 12.0];

/// Returns an empty chunk of a fresh compositor's transaction, and two solid paint slots.
fn chunk_and_slots() -> (RecordedChunk, [TextPaint; 2]) {
    let mut compositor = CompositorImpl::new();
    let (client, _) = compositor.register_client();
    let mut tx = compositor.transaction(client);
    let space = tx.create_transform(TransformNode {
        parent: None,
        local: Matrix4::IDENTITY,
        scroll: None,
        anchor: iceblink::Anchor::Scrolls,
    });
    let mut slot = || {
        let paint = iceblink::Paint::Solid(iceblink::Color([0.0, 0.0, 0.0, 1.0]));
        TextPaint::Slot(PaintKind::SOLID, tx.create_paint(paint))
    };
    let slots = [slot(), slot()];
    let props = ChunkProps {
        space,
        clip: None,
        flags: ChunkFlags::default(),
    };
    let chunk = tx.empty_chunk(ChunkKey::new(1, 0), props);
    (chunk, slots)
}

fn heavy_word(fonts: &mut Fonts, paint: TextPaint) -> Layout {
    let mut text = fonts.typesetter.text("Hollo moon");
    text.style(Property::Family(VARIABLE_FAMILY))
        .style(Property::Size(24.0))
        .style(Property::Paint(paint))
        .span(0..5, Property::Weight(800.0));
    text.build(None, Alignment::Start)
}

#[test]
fn runs_are_recorded_at_the_origin_with_their_coordinates() {
    let mut fonts = Fonts::new();
    let (mut chunk, [paint, _]) = chunk_and_slots();
    let layout = heavy_word(&mut fonts, paint);
    TextRecorder::new(&mut chunk.recorder()).layout(&layout, ORIGIN);

    let runs: Vec<_> = layout
        .items()
        .filter_map(|item| match item {
            TextItem::Run(run) => Some(run),
            TextItem::Line(_) => None,
        })
        .collect();
    assert_eq!(chunk.glyph_runs.len(), runs.len());
    for (recorded, run) in chunk.glyph_runs.iter().zip(&runs) {
        assert_eq!(recorded.font, run.font);
        assert_eq!(&chunk.coords[recorded.coords.indices()], run.coords);
        let moved: Vec<[f32; 2]> = run
            .glyphs
            .iter()
            .map(|glyph| [glyph.x + ORIGIN[0], glyph.y + ORIGIN[1]])
            .collect();
        let glyphs: Vec<[f32; 2]> = chunk.glyphs[recorded.glyphs.indices()]
            .iter()
            .map(|glyph| [glyph.x, glyph.y])
            .collect();
        assert_eq!(glyphs, moved);
    }
    assert!(!chunk.coords.is_empty());
}

#[test]
fn a_paint_slot_is_referenced_once_however_often_it_is_used() {
    let mut fonts = Fonts::new();
    let (mut chunk, [ink, highlight]) = chunk_and_slots();
    let layout = heavy_word(&mut fonts, ink);
    let mut recorder = chunk.recorder();
    let mut text = TextRecorder::new(&mut recorder);
    text.selection(&layout, 0..5, ORIGIN, highlight);
    text.layout(&layout, ORIGIN);
    text.caret(&layout, Position::new(5), 1.0, ORIGIN, highlight);

    let paints = chunk
        .refs
        .iter()
        .filter(|id| matches!(id, AnyId::Paint(_)))
        .count();
    assert_eq!(paints, 2);
    assert!(chunk.glyph_runs.len() > 1);
    assert_eq!(chunk.quads.len(), 2);
}
