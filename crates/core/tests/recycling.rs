//! Recorded arrays move into the store and come back through later transactions.

mod common;

use iceblink::Compositor;
use iceblink::{ChunkKey, PaintRef};

use common::{Scene, props, quad};

#[test]
fn a_replaced_recording_comes_back_without_a_copy() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);

    let mut tx = scene.tx();
    let mut chunk = tx.record(key, props(scene.space));
    for index in 0..32 {
        chunk.quad(quad([index as f32, 0.0, 1.0, 1.0], PaintRef::NONE));
    }
    scene.commit(tx);

    let mut tx = scene.tx();
    let mut chunk = tx.record(key, props(scene.space));
    chunk.quad(quad([0.0, 0.0, 1.0, 1.0], PaintRef::NONE));
    scene.commit(tx);

    // The first recording was replaced, so its arrays are a spare of the next transaction.
    let parts = scene.compositor.transaction(scene.client).into_parts();
    let spare = parts
        .spare
        .iter()
        .find(|chunk| chunk.quads.capacity() >= 32)
        .expect("the replaced recording is recycled");
    assert!(spare.ops.is_empty() && spare.quads.is_empty());
    let address = spare.quads.as_ptr();
    let capacity = spare.quads.capacity();
    let ids = parts.ids.clone();
    let mut tx = iceblink::Transaction::from_recycled(ids, parts);

    // Recording again fills a spare before allocating, the store keeps that array, and replacing
    // the chunk hands the same array back.
    let mut chunk = tx.record(key, props(scene.space));
    for index in 0..capacity {
        chunk.quad(quad([index as f32, 1.0, 1.0, 1.0], PaintRef::NONE));
    }
    scene.commit(tx);

    let mut tx = scene.tx();
    let mut chunk = tx.record(key, props(scene.space));
    chunk.quad(quad([0.0, 0.0, 1.0, 1.0], PaintRef::NONE));
    scene.commit(tx);

    let parts = scene.compositor.transaction(scene.client).into_parts();
    assert!(
        parts
            .spare
            .iter()
            .any(|chunk| chunk.quads.as_ptr() == address),
        "the array recorded into came back unmoved"
    );
}
