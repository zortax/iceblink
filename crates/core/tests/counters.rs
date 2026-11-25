//! The counters of commits and frames, which show the work stays proportional to the change and
//! report content drawn as nothing.

mod common;

use iceblink::Compositor;
use iceblink::{Built, ChunkKey, Color, OrderEntry, Paint, PaintKind, PaintRef};

use common::text::{run, test_font};
use common::{Scene, props, quad, stat};

/// Commits `count` chunks of one quad each, all filled with one paint, draws a frame, and returns
/// the paint.
fn many(scene: &mut Scene, count: u64) -> iceblink::PaintId {
    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
    let mut entries = Vec::new();
    for index in 0..count {
        let key = ChunkKey::new(index, 0);
        let mut chunk = tx.record(key, props(scene.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        let x = (index % 20) as f32 * 10.0;
        let y = (index / 20) as f32 * 10.0;
        chunk.quad(quad([x, y, 8.0, 8.0], fill));
        entries.push(OrderEntry::Chunk(key));
    }
    tx.splice(scene.root, 0, 0, &entries);
    scene.commit(tx);
    scene.draw();
    paint
}

#[test]
fn a_commit_counts_what_it_recorded_and_wrote() {
    let mut scene = Scene::new();
    let paint = many(&mut scene, 100);
    let counters = scene.compositor.counters().commit;
    assert_eq!(counters.chunks_recorded, 100);
    assert_eq!(counters.keys_spliced, 100);
    assert_eq!(counters.paint_writes, 0, "creating a slot is not a write");

    let mut tx = scene.tx();
    tx.set_offset(ChunkKey::new(5, 0), [1.0, 0.0]);
    tx.set_paint(paint, Paint::Solid(Color([0.0, 0.0, 0.0, 1.0])));
    scene.commit(tx);
    let counters = scene.compositor.counters().commit;
    assert_eq!(counters.chunks_recorded, 0);
    assert_eq!(counters.offsets_written, 1);
    assert_eq!(counters.paint_writes, 1);
    assert_eq!(counters.bytes_recorded, 0);
}

#[test]
fn a_frame_draws_only_what_meets_the_damage() {
    let mut scene = Scene::new();
    many(&mut scene, 200);

    let mut tx = scene.tx();
    tx.set_offset(ChunkKey::new(21, 0), [0.0, 1.0]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    let frame = scene.compositor.counters().frame;
    assert_eq!(frame.chunks_drawn, 1);
    assert_eq!(frame.items_drawn, 1);
    assert_eq!(frame.order_nodes_visited, 1);
    assert_eq!(scene.frame.lanes.quads.drawn(), 1);
    assert!(frame.damage_px <= 2 * 8 * 9);
}

#[test]
fn the_chunk_bytes_reported_follow_recordings_lowerings_and_removals() {
    let mut scene = Scene::new();
    many(&mut scene, 40);
    let drawn = scene.compositor.memory().chunk_bytes;
    assert!(drawn > 0);

    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 20, &[]);
    for index in 0..20 {
        tx.remove_chunk(ChunkKey::new(index, 0));
    }
    scene.commit(tx);
    scene.draw();
    let half = scene.compositor.memory().chunk_bytes;
    assert_eq!(half * 2, drawn, "every chunk holds the same bytes");

    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 20, &[]);
    for index in 20..40 {
        tx.remove_chunk(ChunkKey::new(index, 0));
    }
    scene.commit(tx);
    scene.draw();
    assert_eq!(scene.compositor.memory().chunk_bytes, 0);
    assert_eq!(stat(&scene.compositor, "verification_mismatches"), 0);
}

#[test]
fn an_idle_frame_does_no_work() {
    let mut scene = Scene::new();
    many(&mut scene, 50);
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });
}

#[test]
fn a_path_left_out_of_the_damage_is_culled_once() {
    use std::sync::Arc;

    use iceblink::kurbo::Shape as _;
    use iceblink::{MarkFlags, MarkPayload, MarkPlacement, Marks};

    let mut scene = Scene::new();
    // A shape too large for the atlas, drawn as a path over the bottom right of the item.
    let payload = Arc::new(MarkPayload {
        shapes: vec![Arc::new(
            iceblink::kurbo::Rect::new(0.0, 0.0, 300.0, 300.0).to_path(0.1),
        )],
        placements: vec![MarkPlacement {
            x: 150.0,
            y: 150.0,
            shape: 0,
            _pad: 0,
        }],
        ..MarkPayload::default()
    });
    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    let index = chunk.payload(payload);
    chunk.marks(Marks {
        bounds: [0.0, 0.0, 200.0, 200.0],
        paint: fill,
        paint_origin: [0.0; 2],
        origin: [0.0; 2],
        axes: [1.0, 0.0, 0.0, 1.0],
        half_width: 0.0,
        discs: 0,
        boxes: 0,
        vertices: 0,
        placements: 1,
        first: 0,
        payload: index,
        clip: iceblink::ClipRef::INHERIT,
        space: iceblink::SpaceRef::INHERIT,
        flags: MarkFlags::default(),
    });
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    scene.entries += 1;
    scene.draw();

    // Damage at the top left meets the item but not its path.
    scene.add_chunk(ChunkKey::new(2, 0), &[[0.0, 0.0, 2.0, 2.0]]);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.vector_plan.is_empty(), "the path is not drawn");
    assert_eq!(scene.compositor.counters().frame.items_culled, 1);
}

#[test]
fn glyph_runs_drawn_without_a_glyph_rasteriser_are_counted() {
    let mut scene = Scene::new();
    let font = scene.compositor.font_registry().register(test_font());
    let mut tx = scene.tx();
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(scene.space));
    let ink = PaintRef::color([0, 0, 0, 255]);
    run(&mut chunk, font, "Unseen", 12.0, [4.0, 20.0], ink);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    scene.entries += 1;
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(
        scene.compositor.counters().frame.glyph_runs_without_raster,
        1
    );
    scene.done(common::presented());

    scene.add_chunk(ChunkKey::new(2, 0), &[[150.0, 150.0, 10.0, 10.0]]);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(
        scene.compositor.counters().frame.glyph_runs_without_raster,
        0,
        "the run misses the damage"
    );
}
