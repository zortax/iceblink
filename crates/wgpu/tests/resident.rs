//! Resident chunks: inserted once, drawn at offsets, retired, released and reset.

mod common;

use std::sync::Arc;

use common::build::{FrameBuilder, filled, opaque, rect};
use common::device::{draw, plain_renderer, present};
use iceblink::Renderer as _;
use iceblink::{
    ClipEntry, ClipLinkEntry, ClipRef, Color, FRAME_NO_CLIP, FrameClipShape, FrameGradient,
    FrameMarkPayload, FrameMarks, FramePaint, FrameStop, GradientShape, Instance, LoweredChunk,
    MarkFlags, MarkPayload, Marks, PaintRef, Quad, ROUND, SpaceRef, StopEncoding,
};

/// Returns a linear ramp from red to blue across 32 pixels.
fn ramp() -> FramePaint {
    FramePaint::Gradient(FrameGradient {
        shape: GradientShape::Linear {
            start: [0.0, 0.0],
            end: [32.0, 0.0],
        },
        encoding: StopEncoding::SRGB,
        repeat: false,
        stops: Arc::from(vec![
            FrameStop {
                color: [1.0, 0.0, 0.0, 1.0],
                offset: 0.0,
            },
            FrameStop {
                color: [0.0, 0.0, 1.0, 1.0],
                offset: 1.0,
            },
        ]),
    })
}

/// Returns a clip of one rounded rectangle.
fn clip() -> ClipEntry {
    ClipEntry {
        links: [ClipLinkEntry {
            space: 0,
            shape: FrameClipShape::RoundedRect {
                rect: rect(20.0, 20.0, 60.0, 60.0),
                radii: [8.0; 8],
                shapes: ROUND,
            },
        }]
        .into_iter()
        .collect(),
    }
}

/// Returns a quad filled with `paint` and clipped by a clip [`clip`] adds to `builder`.
fn clipped_quad(builder: &mut FrameBuilder, paint: PaintRef) -> Quad {
    let mut quad = filled(rect(8.0, 8.0, 32.0, 32.0), paint);
    quad.clip = builder.clip(clip());
    quad
}

#[test]
fn a_resident_quad_at_an_offset_draws_what_its_moved_copy_draws() {
    let mut renderer = plain_renderer();

    let mut transient = FrameBuilder::new();
    let paint = transient.paint(ramp());
    let quad = clipped_quad(&mut transient, paint);
    transient.quad(quad.moved([24.0, 16.0]));
    let expected = present(&mut renderer, &transient.finish());

    drop(renderer);
    let mut other = plain_renderer();
    let mut resident = FrameBuilder::new();
    let paint = resident.paint(ramp());
    let quad = clipped_quad(&mut resident, paint);
    let chunk = Arc::new(LoweredChunk {
        quads: vec![quad],
        ..LoweredChunk::default()
    });
    resident.insert(7, Arc::clone(&chunk));
    resident.draw_resident_quads(7, [24.0, 16.0], &chunk.quads);
    let actual = present(&mut other, &resident.finish());

    assert_eq!(actual.max_difference(&expected), 0);
    assert!(actual.rgba(40, 30)[2] > 0, "the ramp is drawn");
}

#[test]
fn a_replay_uploads_no_chunk_bytes_and_a_release_keeps_the_picture() {
    let mut renderer = plain_renderer();
    let mut first = FrameBuilder::new();
    let paint = first.solid(opaque(0, 255, 0));
    let quad = filled(rect(16.0, 16.0, 32.0, 32.0), paint);
    let chunk = Arc::new(LoweredChunk {
        quads: vec![quad],
        ..LoweredChunk::default()
    });
    first.insert(1, Arc::clone(&chunk));
    first.draw_resident_quads(1, [0.0, 0.0], &chunk.quads);
    let inserted = draw(&mut renderer, &first.finish()).stats().unwrap();

    assert!(inserted.chunk_bytes_uploaded > 0);
    assert_eq!(inserted.chunks_resident, 1);
    let picture = renderer.read_presented().unwrap();

    let replay = || {
        let mut next = FrameBuilder::next();
        next.draw_resident_quads(1, [0.0, 0.0], &chunk.quads);
        next.finish()
    };
    let replayed = draw(&mut renderer, &replay()).stats().unwrap();

    assert_eq!(
        replayed.chunk_bytes_uploaded, 0,
        "a replay uploads no chunk"
    );

    assert!(renderer.release_idle_resources() > 0);
    let restored = draw(&mut renderer, &replay()).stats().unwrap();
    assert!(
        restored.chunk_bytes_uploaded > 0,
        "a released chunk is uploaded again once"
    );
    assert_eq!(
        renderer.read_presented().unwrap().max_difference(&picture),
        0
    );
    let again = draw(&mut renderer, &replay()).stats().unwrap();
    assert_eq!(again.chunk_bytes_uploaded, 0);
}

#[test]
fn a_reset_drops_every_chunk_and_a_new_insert_draws_again() {
    let mut renderer = plain_renderer();
    let mut first = FrameBuilder::new();
    let paint = first.solid(Color([1.0, 0.0, 0.0, 1.0]));
    let chunk = Arc::new(LoweredChunk {
        quads: vec![filled(rect(0.0, 0.0, 64.0, 64.0), paint)],
        ..LoweredChunk::default()
    });
    first.insert(1, Arc::clone(&chunk));
    first.draw_resident_quads(1, [0.0, 0.0], &chunk.quads);
    draw(&mut renderer, &first.finish());

    let mut reset = FrameBuilder::next();
    reset
        .frame_mut()
        .chunk_notes
        .push(iceblink::ChunkNote::ResetAll);
    reset.insert(2, Arc::clone(&chunk));
    reset.draw_resident_quads(2, [32.0, 32.0], &chunk.quads);
    let stats = draw(&mut renderer, &reset.finish()).stats().unwrap();

    assert_eq!(
        stats.chunks_resident, 1,
        "only the reinserted revision is resident"
    );
    let pixels = renderer.read_presented().unwrap();
    assert_eq!(pixels.rgba(40, 40), [255, 0, 0, 255]);
    assert_eq!(pixels.rgba(8, 8), [0, 0, 0, 0]);
}

/// Returns a ring of marks whose payload is placed relative to `origin`, and its payload.
fn ring(
    paint: PaintRef,
    origin: [f32; 2],
    payload: Arc<MarkPayload>,
) -> (FrameMarks, FrameMarkPayload) {
    let marks = FrameMarks {
        marks: Marks {
            bounds: rect(68.5, 28.5, 24.0, 24.0),
            paint,
            paint_origin: [0.0; 2],
            origin,
            axes: [1.0, 0.0, 0.0, 1.0],
            half_width: 0.0,
            discs: 1,
            boxes: 0,
            vertices: 0,
            placements: 0,
            first: 0,
            payload: 0,
            clip: ClipRef(FRAME_NO_CLIP),
            space: SpaceRef(0),
            flags: MarkFlags::default(),
        },
        page: 0,
    };
    let payload = FrameMarkPayload {
        payload,
        cells: Arc::from(Vec::new()),
    };
    (marks, payload)
}

/// Returns the payload of a ring around `(80.5, 40.5)`, relative to `origin`.
fn ring_payload(origin: [f32; 2]) -> Arc<MarkPayload> {
    Arc::new(MarkPayload {
        discs: vec![[80.5 - origin[0], 40.5 - origin[1], 12.0, 7.5]],
        ..MarkPayload::default()
    })
}

/// Returns `marks` with its bounds and origin moved by `by`.
fn moved_marks(marks: FrameMarks, by: [f32; 2]) -> FrameMarks {
    let mut moved = marks;
    let [x, y, w, h] = moved.marks.bounds;
    moved.marks.bounds = [x + by[0], y + by[1], w, h];
    moved.marks.origin = [moved.marks.origin[0] + by[0], moved.marks.origin[1] + by[1]];
    moved
}

// A resident marks chunk drawn at an offset moves its origin with it, so it draws what a fresh
// encoding at the moved place draws, whether its payload is resident or gathered again after a
// release.
#[test]
fn a_moved_marks_chunk_uploads_nothing_and_draws_as_a_fresh_encoding() {
    for origin in [[0.0, 0.0], [12.0, -6.0]] {
        let (mut renderer, mut control) = common::device::pair();
        let mut builder = FrameBuilder::new();
        let green = builder.solid(Color([0.08, 0.63, 0.35, 1.0]));
        let (marks, payload) = ring(green, origin, ring_payload(origin));
        let chunk = Arc::new(LoweredChunk {
            marks: vec![marks],
            mark_payloads: vec![payload.clone()],
            ..LoweredChunk::default()
        });
        builder.insert(1, Arc::clone(&chunk));
        builder.draw_resident(1, [0.0, 0.0], &chunk);
        let first = present(&mut renderer, &builder.finish());
        assert_eq!(first.rgba(80, 30)[3], 255, "the ring draws");
        assert_eq!(first.rgba(80, 40)[3], 0, "and leaves its hole");

        let moved = || {
            let mut next = FrameBuilder::next();
            next.draw_resident(1, [5.0, 7.0], &chunk);
            next.finish()
        };
        let stats = draw(&mut renderer, &moved()).stats().expect("presented");
        assert_eq!(
            stats.chunk_bytes_uploaded, 0,
            "a moved replay uploads nothing"
        );
        let resident = renderer.read_presented().expect("an offscreen image");
        renderer.release_idle_resources();
        let gathered = present(&mut renderer, &moved());

        let mut fresh = FrameBuilder::new();
        let green = fresh.solid(Color([0.08, 0.63, 0.35, 1.0]));
        let (marks, payload) = ring(green, origin, ring_payload(origin));
        fresh.marks(moved_marks(marks, [5.0, 7.0]), payload);
        let expected = present(&mut control, &fresh.finish());
        assert_eq!(
            resident.max_difference(&expected),
            0,
            "origin {origin:?}: resident"
        );
        assert_eq!(
            gathered.max_difference(&expected),
            0,
            "origin {origin:?}: gathered again"
        );
    }
}

#[test]
fn a_revision_retired_in_the_frame_that_inserts_it_leaves_nothing_resident() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let paint = builder.solid(Color([1.0, 0.0, 0.0, 1.0]));
    let chunk = Arc::new(LoweredChunk {
        quads: vec![filled(rect(0.0, 0.0, 64.0, 64.0), paint)],
        ..LoweredChunk::default()
    });
    builder.insert(1, Arc::clone(&chunk));
    builder.retire(1);

    let stats = draw(&mut renderer, &builder.finish())
        .stats()
        .expect("presented");

    assert_eq!(stats.chunks_resident, 0);
}

#[test]
fn a_payload_shared_by_two_revisions_outlives_its_first_holder() {
    let mut renderer = plain_renderer();
    let payload = ring_payload([0.0; 2]);
    let chunk = |paint: PaintRef| {
        let (marks, payload) = ring(paint, [0.0; 2], Arc::clone(&payload));
        Arc::new(LoweredChunk {
            marks: vec![marks],
            mark_payloads: vec![payload],
            ..LoweredChunk::default()
        })
    };
    let mut builder = FrameBuilder::new();
    let green = builder.solid(Color([0.08, 0.63, 0.35, 1.0]));
    let first = chunk(green);
    builder.insert(1, Arc::clone(&first));
    builder.draw_resident(1, [0.0, 0.0], &first);
    let picture = present(&mut renderer, &builder.finish());

    let second = chunk(green);
    let mut next = FrameBuilder::next();
    next.insert(2, Arc::clone(&second));
    next.retire(1);
    next.draw_resident(2, [0.0, 0.0], &second);
    let drawn = present(&mut renderer, &next.finish());

    assert_eq!(
        drawn.max_difference(&picture),
        0,
        "the payload outlived its first holder"
    );

    let mut again = FrameBuilder::next();
    again.draw_resident(2, [3.0, 0.0], &second);
    let stats = draw(&mut renderer, &again.finish())
        .stats()
        .expect("presented");
    assert_eq!(
        stats.chunk_bytes_uploaded, 0,
        "the shared payload stays resident"
    );
}

#[test]
fn a_hundred_moved_replays_hold_the_chunk_memory_still() {
    let mut renderer = plain_renderer();
    let mut builder = FrameBuilder::new();
    let green = builder.solid(Color([0.08, 0.63, 0.35, 1.0]));
    let (marks, payload) = ring(green, [0.0; 2], ring_payload([0.0; 2]));
    let chunk = Arc::new(LoweredChunk {
        quads: vec![filled(rect(8.0, 8.0, 16.0, 16.0), green)],
        marks: vec![marks],
        mark_payloads: vec![payload],
        ..LoweredChunk::default()
    });
    builder.insert(1, Arc::clone(&chunk));
    builder.draw_resident(1, [0.0, 0.0], &chunk);
    draw(&mut renderer, &builder.finish());

    let mut held = 0;
    for step in 0..110 {
        // The first frames warm the upload staging, which the chunk memory includes.
        if step == 10 {
            held = renderer.memory().chunks;
        }
        let mut next = FrameBuilder::next();
        next.draw_resident(1, [(step % 7) as f32, (step % 5) as f32], &chunk);
        let stats = draw(&mut renderer, &next.finish())
            .stats()
            .expect("presented");
        assert_eq!(
            stats.chunk_bytes_uploaded, 0,
            "frame {step} uploads nothing"
        );
        // A frame still in flight makes the staging belt open another chunk, which it keeps for
        // a while, so the device finishes each frame before the next one starts.
        renderer.gpu().wait();
    }

    assert_eq!(
        renderer.memory().chunks,
        held,
        "the chunk memory holds still"
    );
}
