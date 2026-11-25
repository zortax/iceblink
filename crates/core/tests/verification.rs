//! Verification mode: hint checks and the comparison with a full rebuild.

mod common;

use iceblink::Compositor;
use iceblink::{Built, ChunkKey, CompositorEvent, PaintRef};

use common::{Scene, props, quad, rect};

/// Commits chunk `key` recorded with one quad per rectangle and the damage hint `hint`.
fn record(scene: &mut Scene, key: ChunkKey, rects: &[[f32; 4]], hint: Option<[f32; 4]>) {
    let mut tx = scene.tx();
    let mut chunk = tx.record(key, props(scene.space));
    for &bounds in rects {
        chunk.quad(quad(bounds, PaintRef::NONE));
    }
    if let Some(hint) = hint {
        chunk.hint(hint);
    }
    scene.commit(tx);
}

#[test]
fn a_hint_cuts_the_damage_of_a_recording() {
    let mut scene = Scene::new();
    // Verification would find the hint too tight and damage the full diff.
    scene.compositor.set_verification(false);
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[0.0, 0.0, 100.0, 10.0]]);
    scene.draw();
    record(
        &mut scene,
        key,
        &[[0.0, 0.0, 100.0, 20.0]],
        Some([0.0, 10.0, 100.0, 10.0]),
    );
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(scene.damage(), Some(vec![rect(0, 10, 100, 10)]));
}

#[test]
fn a_violated_hint_is_reported_and_fully_damaged() {
    let mut scene = Scene::new();
    scene.compositor.set_verification(true);
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[0.0, 0.0, 10.0, 10.0]]);
    scene.draw();

    record(
        &mut scene,
        key,
        &[[50.0, 50.0, 10.0, 10.0]],
        Some([0.0, 0.0, 10.0, 10.0]),
    );
    assert_eq!(scene.compositor.counters().hint_violations, 1);
    let mut events = Vec::new();
    scene.compositor.drain_events(&mut events);
    assert_eq!(
        events,
        vec![CompositorEvent::HintViolated {
            client: scene.client,
            chunk: key,
        }]
    );
    assert_eq!(scene.build(), Built::Frame);
    let held = scene.damage().expect("the damage is not full");
    assert!(
        held.iter()
            .any(|rect| rect.contains_rect(common::rect(50, 50, 10, 10)))
    );
}

#[test]
fn cached_inks_match_a_full_rebuild() {
    let mut scene = Scene::new();
    scene.compositor.set_verification(true);
    scene.add_chunk(ChunkKey::new(1, 0), &[[0.0, 0.0, 10.0, 10.0]]);
    scene.add_chunk(ChunkKey::new(2, 0), &[[20.0, 0.0, 10.0, 10.0]]);
    let mut tx = scene.tx();
    tx.set_offset(ChunkKey::new(2, 0), [3.0, 4.0]);
    scene.commit(tx);
    scene.draw();

    let mut stats = Vec::new();
    scene
        .compositor
        .extra_stats(&mut |name, value| stats.push((name, value)));
    assert!(stats.contains(&("verification_mismatches", 0)));
}
