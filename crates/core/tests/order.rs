//! Order tree splices and the per-node ink cache that lets a frame skip subtrees.

mod common;

use iceblink::Compositor;
use iceblink::{Built, ChunkKey, OrderEntry, OrderNode, PaintRef};

use common::{Scene, props, quad};

/// Adds `count` order nodes under the root, each holding one chunk with one quad in its own
/// column, and returns their ids.
fn columns(scene: &mut Scene, count: u32) -> Vec<iceblink::OrderId> {
    let mut tx = scene.tx();
    let mut nodes = Vec::new();
    for index in 0..count {
        let node = tx.create_order(OrderNode::default());
        let key = ChunkKey::new(u64::from(index), 0);
        let mut chunk = tx.record(key, props(scene.space));
        chunk.quad(quad([index as f32 * 10.0, 0.0, 8.0, 8.0], PaintRef::NONE));
        tx.splice(node, 0, 0, &[OrderEntry::Chunk(key)]);
        nodes.push(node);
    }

    let entries: Vec<OrderEntry> = nodes.iter().map(|node| OrderEntry::Node(*node)).collect();
    tx.splice(scene.root, 0, 0, &entries);
    scene.commit(tx);
    scene.draw();
    nodes
}

#[test]
fn a_frame_skips_subtrees_whose_ink_misses_the_damage() {
    let mut scene = Scene::new();
    columns(&mut scene, 10);

    let mut tx = scene.tx();
    tx.set_offset(ChunkKey::new(3, 0), [0.0, 1.0]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    let frame = scene.compositor.counters().frame;
    assert_eq!(
        frame.order_nodes_visited, 2,
        "the root and the changed column"
    );
    assert_eq!(frame.order_nodes_skipped, 9);
    assert_eq!(frame.chunks_drawn, 1);
    assert_eq!(frame.items_drawn, 1);
}

#[test]
fn a_splice_moves_a_subtree_and_keeps_the_cache_current() {
    let mut scene = Scene::new();
    let nodes = columns(&mut scene, 4);

    // Move the last column to the front of the sequence.
    let mut tx = scene.tx();
    tx.splice(scene.root, 3, 1, &[]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(nodes[3])]);
    scene.commit(tx);
    assert_eq!(scene.compositor.counters().commit.keys_spliced, 2);
    assert_eq!(scene.build(), Built::Frame);
    scene.done(common::presented());

    // A change in the moved column is still found through the cache.
    let mut tx = scene.tx();
    tx.set_offset(ChunkKey::new(3, 0), [0.0, 2.0]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    let frame = scene.compositor.counters().frame;
    assert_eq!(frame.chunks_drawn, 1);
    assert_eq!(frame.order_nodes_skipped, 3);
}

#[test]
fn a_node_inserted_before_its_removal_moves_to_one_place() {
    let mut scene = Scene::new();
    let nodes = columns(&mut scene, 4);

    // The last column is held twice between the two splices.
    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(nodes[3])]);
    tx.splice(scene.root, 4, 1, &[]);
    scene.commit(tx);
    scene.draw();

    // Held once again, it moves into another column.
    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 1, &[]);
    tx.splice(nodes[0], 1, 0, &[OrderEntry::Node(nodes[3])]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_offset(ChunkKey::new(3, 0), [0.0, 2.0]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(scene.compositor.counters().frame.chunks_drawn, 1);
}

#[test]
fn a_chunk_key_moved_between_nodes_is_held_once() {
    let mut scene = Scene::new();
    let nodes = columns(&mut scene, 3);
    let key = ChunkKey::new(0, 0);

    let mut tx = scene.tx();
    tx.splice(nodes[0], 0, 1, &[]);
    tx.splice(nodes[1], 1, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    scene.draw();

    // Held once again, it moves on to the last column.
    let mut tx = scene.tx();
    tx.splice(nodes[2], 1, 0, &[OrderEntry::Chunk(key)]);
    tx.splice(nodes[1], 1, 1, &[]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_offset(key, [0.0, 2.0]);
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(scene.compositor.counters().frame.chunks_drawn, 1);
}

#[test]
fn an_entry_may_name_a_chunk_recorded_later() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Idle { wake_ns: None });

    let mut tx = scene.tx();
    let mut chunk = tx.record(key, props(scene.space));
    chunk.quad(quad([0.0, 0.0, 8.0, 8.0], PaintRef::NONE));
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert_eq!(scene.frame.lanes.quads.drawn(), 1);
}
