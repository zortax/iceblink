//! An identical re-recording adds no damage and keeps the chunk's resident revision.

mod common;

use iceblink::Compositor;
use iceblink::{Built, ChunkKey, ChunkNote, Color, OrderEntry, Paint, PaintKind, Quad};

use common::{Scene, props, quad, rect};

/// Commits chunk `key` with a quad at each of `rects` filled with `paint`, and places it first
/// in the root if `place`.
fn record(
    scene: &mut Scene,
    key: ChunkKey,
    paint: iceblink::PaintId,
    rects: &[[f32; 4]],
    place: bool,
) {
    let mut tx = scene.tx();
    let mut chunk = tx.record(key, props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    for &bounds in rects {
        chunk.quad(quad(bounds, fill));
    }
    if place {
        tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
    }
    scene.commit(tx);
}

fn paint(scene: &mut Scene) -> iceblink::PaintId {
    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 0.0, 0.0, 1.0])));
    scene.commit(tx);
    paint
}

#[test]
fn an_identical_recording_adds_no_damage() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    let fill = paint(&mut scene);
    record(
        &mut scene,
        key,
        fill,
        &[[0.0, 0.0, 10.0, 10.0], [50.0, 0.0, 10.0, 10.0]],
        true,
    );
    assert_eq!(scene.draw(), Built::Frame);
    let revision = scene.frame.drawn[0].revision;

    record(
        &mut scene,
        key,
        fill,
        &[[0.0, 0.0, 10.0, 10.0], [50.0, 0.0, 10.0, 10.0]],
        false,
    );
    assert_eq!(scene.compositor.counters().commit.chunks_identical, 1);
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });

    // The lowering carried over, so drawing the chunk again inserts nothing.
    let mut tx = scene.tx();
    tx.set_offset(key, [0.0, 100.0]);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert_eq!(scene.frame.drawn[0].revision, revision);
    assert!(
        !scene
            .frame
            .chunk_notes
            .iter()
            .any(|note| matches!(note, ChunkNote::Inserted { .. })),
        "nothing is uploaded again"
    );
}

#[test]
fn a_changed_recording_damages_only_the_positions_that_differ() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    let fill = paint(&mut scene);
    record(
        &mut scene,
        key,
        fill,
        &[[0.0, 0.0, 10.0, 10.0], [50.0, 0.0, 10.0, 10.0]],
        true,
    );
    scene.draw();

    record(
        &mut scene,
        key,
        fill,
        &[[0.0, 0.0, 10.0, 10.0], [50.0, 20.0, 10.0, 10.0]],
        false,
    );
    assert_eq!(scene.compositor.counters().commit.chunks_identical, 0);
    assert_eq!(scene.build(), Built::Frame);
    let mut rects = scene.damage().expect("the damage is not full");
    rects.sort_by_key(|rect| rect.origin.y);
    assert_eq!(rects, vec![rect(50, 0, 10, 10), rect(50, 20, 10, 10)]);
}

#[test]
fn a_recording_of_a_moved_chunk_is_never_identical() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    let fill = paint(&mut scene);
    record(&mut scene, key, fill, &[[0.0, 0.0, 10.0, 10.0]], true);
    let mut tx = scene.tx();
    tx.set_offset(key, [30.0, 0.0]);
    scene.commit(tx);
    scene.draw();

    // Recording resets the offset to zero, which moves the pixels back.
    record(&mut scene, key, fill, &[[0.0, 0.0, 10.0, 10.0]], false);
    assert_eq!(scene.compositor.counters().commit.chunks_identical, 0);
    assert_eq!(scene.build(), Built::Frame);
    let mut rects = scene.damage().expect("the damage is not full");
    rects.sort_by_key(|rect| rect.origin.x);
    assert_eq!(rects, vec![rect(0, 0, 10, 10), rect(30, 0, 10, 10)]);
}

#[test]
fn a_solid_quad_that_only_grows_damages_the_band_it_gained() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    let fill = paint(&mut scene);
    record(&mut scene, key, fill, &[[0.0, 0.0, 100.0, 100.0]], true);
    assert_eq!(scene.draw(), Built::Frame);

    record(&mut scene, key, fill, &[[0.0, 0.0, 108.0, 100.0]], false);
    assert_eq!(scene.draw(), Built::Frame);
    // The right edge moved from 100 to 108, and coverage ramps half a pixel on either side.
    assert_eq!(scene.damage(), Some(vec![rect(99, 0, 9, 100)]));
}

#[test]
fn a_rounded_quad_whose_side_edges_move_damages_less_than_its_ink() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    let fill = paint(&mut scene);
    let record_rounded = |scene: &mut Scene, bounds: [f32; 4], place: bool| {
        let mut tx = scene.tx();
        let mut chunk = tx.record(key, props(scene.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, fill);
        chunk.quad(Quad {
            radii: [10.0; 8],
            ..quad(bounds, fill)
        });
        if place {
            tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
        }
        scene.commit(tx);
    };
    record_rounded(&mut scene, [0.0, 0.0, 100.0, 100.0], true);
    assert_eq!(scene.draw(), Built::Frame);

    record_rounded(&mut scene, [4.0, 0.0, 100.0, 100.0], false);
    assert_eq!(scene.draw(), Built::Frame);
    let rects = scene.damage().expect("the damage is not full");
    let area: i32 = rects
        .iter()
        .map(|held| held.size.width * held.size.height)
        .sum();
    assert!(
        area < 104 * 100 / 2,
        "the edges and corners moved, most of the middle did not: {rects:?}"
    );
}

#[test]
fn a_gradient_quad_that_grows_damages_its_whole_ink() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    let mut tx = scene.tx();
    let gradient = tx.create_paint(common::stories::ramp());
    scene.commit(tx);
    let record_gradient = |scene: &mut Scene, width: f32, place: bool| {
        let mut tx = scene.tx();
        let mut chunk = tx.record(key, props(scene.space));
        let fill = chunk.paint_ref(PaintKind::GRADIENT, gradient);
        chunk.quad(quad([0.0, 0.0, width, 100.0], fill));
        if place {
            tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
        }
        scene.commit(tx);
    };
    record_gradient(&mut scene, 100.0, true);
    assert_eq!(scene.draw(), Built::Frame);

    record_gradient(&mut scene, 108.0, false);
    assert_eq!(scene.draw(), Built::Frame);
    assert_eq!(scene.damage(), Some(vec![rect(0, 0, 108, 100)]));
}
