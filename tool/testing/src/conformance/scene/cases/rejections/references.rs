//! References that must resolve: live ids, recorded chunks and outputs, and removals only of
//! objects nothing uses.

use iceblink::{
    Color, CommitError, IdSpace, OrderEntry, OrderNode, OutputId, Paint, PaintKind, RejectReason,
    Transaction,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, props, record_quads};
use crate::conformance::scene::cases::rejections::{RED, quad_with, rejects};
use crate::conformance::scene::stage::Stage;

/// Every id a transaction refers to must be live with its current generation, and minted by the
/// id space the client registered with.
pub fn stale_ids(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    stage.commit(tx);
    let mut tx = stage.tx();
    tx.remove(paint);
    stage.commit(tx);

    let mut tx = stage.tx();
    tx.set_paint(paint, Paint::Solid(Color([0.0, 1.0, 0.0, 1.0])));
    rejects(
        &mut stage,
        tx,
        RejectReason::StaleId,
        "a write to a removed paint",
    );

    let mut tx = stage.tx();
    let mut chunk = tx.record(key(1), stage.props());
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad_with(fill));
    rejects(
        &mut stage,
        tx,
        RejectReason::StaleId,
        "a chunk referring to a removed paint",
    );

    // The client's ids come from the space it registered with, never from a new one.
    let mut foreign = Transaction::new(IdSpace::new(stage.client));
    foreign.create_order(OrderNode::default());
    let result = stage.try_commit(foreign);
    assert!(
        matches!(
            result,
            Err(CommitError {
                reason: RejectReason::StaleId,
                ..
            })
        ),
        "a transaction of a foreign id space: expected a rejection for StaleId, got {result:?}"
    );
}

/// A removed object may not stay named by what remains.
pub fn in_use(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    let mut chunk = tx.record(key(1), props(stage.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad_with(fill));
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);

    let mut tx = stage.tx();
    tx.remove(paint);
    rejects(
        &mut stage,
        tx,
        RejectReason::InUse,
        "removing a paint a chunk names",
    );

    let mut tx = stage.tx();
    let space = tx.create_transform(identity(None));
    record_quads(
        &mut tx,
        key(2),
        props(space),
        &[([0.0, 0.0, 8.0, 8.0], RED)],
    );
    stage.commit(tx);
    let mut tx = stage.tx();
    tx.remove(space);
    rejects(
        &mut stage,
        tx,
        RejectReason::InUse,
        "removing a space a chunk names",
    );

    // Removing the chunk and what it names together is legal.
    let mut tx = stage.tx();
    tx.remove_chunk(key(1));
    tx.remove(paint);
    stage.commit(tx);
}

/// Chunk commands name chunks that exist.
pub fn unknown_chunks(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let mut tx = stage.tx();
    tx.set_offset(key(7), [1.0, 0.0]);
    rejects(
        &mut stage,
        tx,
        RejectReason::UnknownChunk,
        "moving a chunk never recorded",
    );

    let mut tx = stage.tx();
    tx.remove_chunk(key(7));
    rejects(
        &mut stage,
        tx,
        RejectReason::UnknownChunk,
        "removing a chunk never recorded",
    );

    let mut tx = stage.tx();
    tx.set_props(key(7), stage.props());
    rejects(
        &mut stage,
        tx,
        RejectReason::UnknownChunk,
        "new props for a chunk never recorded",
    );
}

/// Output commands name outputs that exist.
pub fn unknown_outputs(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let missing = OutputId(stage.output.0.wrapping_add(1000));
    let mut tx = stage.tx();
    tx.set_root(missing, Some(stage.root));
    rejects(
        &mut stage,
        tx,
        RejectReason::UnknownOutput,
        "a root for no output",
    );

    let mut tx = stage.tx();
    tx.declare_damage(missing, Default::default());
    rejects(
        &mut stage,
        tx,
        RejectReason::UnknownOutput,
        "declared damage of no output",
    );
}
