//! Frame transcripts: stability, resolution through renderer state, and the golden.

mod common;

use std::path::Path;
use std::sync::Arc;

use iceblink::{BatchKind, ChunkNote, DrawnChunk, Frame, ItemSource, LoweredChunk, PaintKind};
use iceblink_testing::assert_valid_frame;
use iceblink_testing::capture::{Mirror, transcript};
use iceblink_testing::golden::assert_text;

use crate::common::capture::{base, batch, kitchen_sink, paint, quad};

/// The transcript of `frame` drawn by a renderer that holds nothing yet.
fn first_transcript(frame: &Frame) -> transcript::Transcript {
    let mut mirror = Mirror::new();
    mirror.apply(frame);
    transcript::of(frame, &mirror)
}

#[test]
fn the_kitchen_sink_is_a_valid_frame_and_matches_its_golden() {
    let frame = kitchen_sink();
    assert_valid_frame(&frame);
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/kitchen_sink.txt");
    assert_text(&golden, first_transcript(&frame).as_str());
}

#[test]
fn a_transcript_is_the_same_on_every_rendering() {
    let first = first_transcript(&kitchen_sink());
    for _ in 0..8 {
        assert_eq!(first_transcript(&kitchen_sink()), first);
    }
}

#[test]
fn a_resident_instance_draws_like_the_same_transient_one() {
    let mut transient = base();
    let mut moved = quad([12.0, 4.0, 8.0, 8.0]);
    moved.paint_origin = [10.0, 0.0];
    transient.lanes.quads.items.push(moved);
    transient.lanes.quads.sources.push(ItemSource::transient(0));
    batch(&mut transient, BatchKind::Quads, 0..1);

    let mut resident = base();
    resident.chunk_notes.push(ChunkNote::Inserted {
        revision: 3,
        chunk: Arc::new(LoweredChunk {
            quads: vec![quad([2.0, 4.0, 8.0, 8.0])],
            ..LoweredChunk::default()
        }),
    });
    resident.drawn.push(DrawnChunk {
        revision: 3,
        offset: [10.0, 0.0],
    });
    resident
        .lanes
        .quads
        .sources
        .push(ItemSource::resident(0, 0));
    batch(&mut resident, BatchKind::Quads, 0..1);

    let drawing = |frame: &Frame| {
        let mut mirror = Mirror::new();
        mirror.apply(frame);
        transcript::drawing(frame, &mirror)
    };
    assert_eq!(drawing(&transient), drawing(&resident));
    assert_ne!(first_transcript(&transient), first_transcript(&resident));
}

#[test]
fn a_slot_the_renderer_lacks_is_shown_as_missing() {
    let mut frame = base();
    let mut stray = quad([0.0, 0.0, 8.0, 8.0]);
    stray.fill = paint(PaintKind::SOLID, 7);
    frame.lanes.quads.items.push(stray);
    frame.lanes.quads.sources.push(ItemSource::transient(0));
    frame.lanes.quads.sources.push(ItemSource::resident(4, 0));
    batch(&mut frame, BatchKind::Quads, 0..2);

    let text = first_transcript(&frame).into_string();
    assert!(text.contains("fill=solid#7 <missing>"), "{text}");
    assert!(text.contains("<missing drawn chunk 4>"), "{text}");
}

#[test]
fn a_paint_of_another_kind_is_shown_as_the_mismatch() {
    let mut frame = base();
    let mut wrong = quad([0.0, 0.0, 8.0, 8.0]);
    wrong.fill = paint(PaintKind::GRADIENT, 0);
    frame.lanes.quads.items.push(wrong);
    frame.lanes.quads.sources.push(ItemSource::transient(0));
    batch(&mut frame, BatchKind::Quads, 0..1);

    let text = first_transcript(&frame).into_string();
    assert!(
        text.contains("fill=<mismatched> declared=gradient entry=solid"),
        "{text}"
    );
}
