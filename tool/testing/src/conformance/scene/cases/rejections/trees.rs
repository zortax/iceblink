//! The trees of nodes: cycles, entries placed twice, and splices out of range.

use iceblink::{OrderEntry, OrderNode, RejectReason};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key};
use crate::conformance::scene::cases::rejections::rejects;
use crate::conformance::scene::stage::Stage;

/// Transform, clip and order parents form trees.
pub fn cycles(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let parent = tx.create_transform(identity(None));
    let child = tx.create_transform(identity(Some(parent)));
    let outer = tx.create_order(OrderNode::default());
    let inner = tx.create_order(OrderNode::default());
    tx.splice(outer, 0, 0, &[OrderEntry::Node(inner)]);
    stage.commit(tx);

    let mut tx = stage.tx();
    tx.set_transform_node(parent, identity(Some(child)));
    rejects(
        &mut stage,
        tx,
        RejectReason::Cycle,
        "a transform under its own child",
    );

    let mut tx = stage.tx();
    tx.splice(inner, 0, 0, &[OrderEntry::Node(outer)]);
    rejects(
        &mut stage,
        tx,
        RejectReason::Cycle,
        "an order node in its own subtree",
    );
}

/// An order node and a chunk key appear in at most one place.
pub fn order_conflicts(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let node = tx.create_order(OrderNode::default());
    stage.append(&mut tx, OrderEntry::Node(node));
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);

    let mut tx = stage.tx();
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key(1))]);
    rejects(
        &mut stage,
        tx,
        RejectReason::OrderConflict,
        "a chunk key in two places",
    );

    let mut tx = stage.tx();
    let other = tx.create_order(OrderNode::default());
    tx.splice(other, 0, 0, &[OrderEntry::Node(node)]);
    rejects(
        &mut stage,
        tx,
        RejectReason::OrderConflict,
        "an order node in two places",
    );
}

/// A splice stays inside its sequence.
pub fn splices_out_of_range(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let node = tx.create_order(OrderNode::default());
    stage.commit(tx);

    let mut tx = stage.tx();
    tx.splice(node, 1, 0, &[OrderEntry::Chunk(key(1))]);
    rejects(
        &mut stage,
        tx,
        RejectReason::SpliceOutOfRange,
        "inserting past the end",
    );

    let mut tx = stage.tx();
    tx.splice(node, 0, 1, &[]);
    rejects(
        &mut stage,
        tx,
        RejectReason::SpliceOutOfRange,
        "removing past the end",
    );
}
