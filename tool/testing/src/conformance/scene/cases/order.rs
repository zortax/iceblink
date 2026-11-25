//! Paint order: the order tree's traversal decides which of two overlapping items is drawn first.

use iceblink::{OrderEntry, OrderNode};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, props, record_quads};
use crate::conformance::scene::stage::Stage;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const GREY: [u8; 4] = [128, 128, 128, 255];

/// Overlapping items are drawn in the depth-first order of the order tree, and in record order
/// within a chunk.
pub fn paint_order(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let node = tx.create_order(OrderNode::default());
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[
            ([10.0, 10.0, 40.0, 40.0], RED),
            ([20.0, 20.0, 40.0, 40.0], GREY),
        ],
    );
    record_quads(
        &mut tx,
        key(2),
        stage.props(),
        &[([30.0, 30.0, 40.0, 40.0], GREEN)],
    );
    record_quads(
        &mut tx,
        key(3),
        stage.props(),
        &[([25.0, 25.0, 40.0, 40.0], BLUE)],
    );
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.append(&mut tx, OrderEntry::Node(node));
    stage.append(&mut tx, OrderEntry::Chunk(key(3)));
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key(2))]);
    stage.commit(tx);

    let picture = stage.redraw();
    let order = [RED, GREY, GREEN, BLUE].map(|color| picture.position(color));
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "overlapping quads are drawn in paint order: red, grey, green, blue at {order:?}"
    );

    // Moving the last chunk to the front of the root's sequence draws it first.
    let mut tx = stage.tx();
    tx.splice(stage.root, 2, 1, &[]);
    tx.splice(stage.root, 0, 0, &[OrderEntry::Chunk(key(3))]);
    stage.commit(tx);
    stage.draw();
    let picture = stage.redraw();
    let order = [BLUE, RED, GREY, GREEN].map(|color| picture.position(color));
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "after a splice the moved chunk is drawn first: blue, red, grey, green at {order:?}"
    );
}

/// Only what the output's root reaches is drawn: an entry for an unrecorded chunk draws nothing,
/// a recorded chunk without an entry is not drawn, and an output without a root draws nothing.
pub fn what_the_root_reaches(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    record_quads(
        &mut tx,
        key(2),
        stage.props(),
        &[([10.0, 10.0, 20.0, 20.0], GREEN)],
    );
    stage.commit(tx);
    let picture = stage.redraw();
    assert!(
        picture.quads.is_empty(),
        "an unrecorded entry and an unplaced chunk draw nothing, but {} quads were drawn",
        picture.quads.len()
    );

    let mut tx = stage.tx();
    record_quads(
        &mut tx,
        key(1),
        stage.props(),
        &[([40.0, 40.0, 20.0, 20.0], RED)],
    );
    stage.commit(tx);
    stage.draw();
    assert_eq!(
        stage.redraw().quads_of(RED).len(),
        1,
        "recording the chunk an entry names draws it"
    );

    let mut tx = stage.tx();
    tx.set_root(stage.output, None);
    stage.commit(tx);
    stage.draw();
    assert!(
        stage.redraw().quads.is_empty(),
        "an output without a root draws nothing"
    );
}

/// An output has one root, and the `set_root` of any client replaces it with a root of its own.
pub fn roots_of_clients(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    stage.add_quads(key(1), &[([0.0, 0.0, 16.0, 16.0], RED)]);

    let (other, _ids) = stage.compositor.register_client();
    let mut tx = stage.compositor.transaction(other);
    let space = tx.create_transform(identity(None));
    let root = tx.create_order(OrderNode::default());
    record_quads(
        &mut tx,
        key(1),
        props(space),
        &[([8.0, 8.0, 16.0, 16.0], GREEN)],
    );
    tx.splice(root, 0, 0, &[OrderEntry::Chunk(key(1))]);
    tx.set_root(stage.output, Some(root));
    stage.commit(tx);
    let picture = stage.redraw();
    assert!(
        picture.quads_of(RED).is_empty() && picture.quads_of(GREEN).len() == 1,
        "the output draws the root the second client set, and only that"
    );

    let mut tx = stage.tx();
    tx.set_root(stage.output, None);
    stage.commit(tx);
    let picture = stage.redraw();
    assert!(
        picture.quads.is_empty(),
        "a root cleared by the first client draws nothing"
    );
}
