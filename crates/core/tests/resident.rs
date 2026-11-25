//! Resident chunk notes and drawn offsets.

mod common;

use iceblink::Compositor;
use iceblink::{
    Built, ChunkKey, ChunkNote, Color, FrameOutcome, ItemSource, Paint, PaintKind,
    RenderCapabilities,
};

use common::{Scene, props, quad};

/// Returns the revisions the last frame inserts.
fn inserted(scene: &Scene) -> Vec<u64> {
    scene
        .frame
        .chunk_notes
        .iter()
        .filter_map(|note| match note {
            ChunkNote::Inserted { revision, .. } => Some(*revision),
            _ => None,
        })
        .collect()
}

/// Returns the revisions the last frame retires.
fn retired(scene: &Scene) -> Vec<u64> {
    scene
        .frame
        .chunk_notes
        .iter()
        .filter_map(|note| match note {
            ChunkNote::Retired { revision } => Some(*revision),
            _ => None,
        })
        .collect()
}

#[test]
fn a_drawn_chunk_is_inserted_once_and_drawn_from_its_revision() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0], [40.0, 10.0, 20.0, 20.0]]);
    assert_eq!(scene.draw(), Built::Frame);
    let revision = inserted(&scene);
    assert_eq!(revision.len(), 1);
    assert_eq!(scene.frame.drawn.len(), 1);
    assert_eq!(scene.frame.drawn[0].revision, revision[0]);
    assert_eq!(
        scene.frame.lanes.quads.sources,
        vec![ItemSource {
            chunk: 0,
            index: 0,
            len: 2
        }],
        "the chunk's two quads are drawn as one run"
    );
    assert!(
        scene.frame.lanes.quads.items.is_empty(),
        "resident instances are not copied"
    );
}

#[test]
fn an_offset_moves_the_drawn_chunk_and_uploads_nothing() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0]]);
    scene.draw();
    let revision = scene.frame.drawn[0].revision;

    let mut tx = scene.tx();
    tx.set_offset(key, [5.0, 7.0]);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert!(inserted(&scene).is_empty());
    assert_eq!(scene.frame.drawn[0].revision, revision);
    assert_eq!(scene.frame.drawn[0].offset, [5.0, 7.0]);
}

#[test]
fn a_replaced_chunk_retires_its_old_revision_in_the_frame_that_draws_the_new_one() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0]]);
    scene.draw();
    let old = scene.frame.drawn[0].revision;

    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color([0.0, 1.0, 0.0, 1.0])));
    let mut chunk = tx.record(key, props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([10.0, 10.0, 30.0, 20.0], fill));
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    let new = inserted(&scene);
    assert_eq!(new.len(), 1);
    assert_ne!(new[0], old);
    assert_eq!(retired(&scene), vec![old]);
    assert!(
        matches!(
            scene.frame.chunk_notes.first(),
            Some(ChunkNote::Retired { .. })
        ),
        "the retirement comes before the insertion"
    );
}

#[test]
fn a_recovered_renderer_gets_every_drawn_revision_again() {
    let mut scene = Scene::new();
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 20.0, 20.0]]);
    scene.draw();
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });

    scene.add_chunk(ChunkKey::new(2, 0), &[[60.0, 10.0, 20.0, 20.0]]);
    assert_eq!(scene.build(), Built::Frame);
    scene.done(FrameOutcome::Recovered);
    assert_eq!(scene.build(), Built::Frame);
    assert!(matches!(
        scene.frame.chunk_notes.first(),
        Some(ChunkNote::ResetAll)
    ));
    assert_eq!(inserted(&scene).len(), 2);
}

#[test]
fn a_renderer_without_residency_gets_moved_transient_copies() {
    let mut scene = Scene::with_caps(RenderCapabilities::MINIMAL);
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0]]);
    let mut tx = scene.tx();
    tx.set_offset(key, [5.0, 0.0]);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert!(scene.frame.chunk_notes.is_empty());
    assert!(scene.frame.drawn.is_empty());
    assert_eq!(
        scene.frame.lanes.quads.sources,
        vec![ItemSource::transient(0)]
    );
    assert_eq!(
        scene.frame.lanes.quads.items[0].bounds,
        [15.0, 10.0, 20.0, 20.0]
    );
}

#[test]
fn forgetting_caches_resets_resident_chunks() {
    let mut scene = Scene::new();
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 20.0, 20.0]]);
    scene.draw();
    scene.compositor.forget_caches();
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.damage.draw.is_full());
    assert!(matches!(
        scene.frame.chunk_notes.first(),
        Some(ChunkNote::ResetAll)
    ));
    assert_eq!(inserted(&scene).len(), 1);
    let memory = scene.compositor.memory();
    assert_eq!((memory.atlas_bytes, memory.atlas_pinned_bytes), (0, 0));
}
