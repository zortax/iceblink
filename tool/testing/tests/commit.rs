//! Removals of objects a chunk still names.

mod common;

use iceblink::Compositor;
use iceblink::{ChunkKey, ClipNode, ClipShape, Paint, PaintKind, RejectReason, Transaction};

use crate::common::naive::{Harness, quad};

/// Commits `transaction`, which must be rejected, and returns the reason.
fn rejected(harness: &mut Harness, transaction: Transaction) -> RejectReason {
    harness
        .compositor
        .commit(transaction)
        .expect_err("the transaction is rejected")
        .reason
}

#[test]
fn removing_what_a_chunk_still_names_is_rejected() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let paint = transaction.create_paint(Paint::Solid(iceblink::Color([1.0; 4])));
    let props = harness.props();
    let mut recorder = transaction.record(ChunkKey::new(1, 0), props);
    let mut item = quad([0.0, 0.0, 8.0, 8.0], [9; 4]);
    item.fill = recorder.paint_ref(PaintKind::SOLID, paint);
    recorder.quad(item);
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    transaction.remove(paint);
    let error = harness.compositor.commit(transaction).unwrap_err();
    assert_eq!(error.reason, RejectReason::InUse);
    assert_eq!(error.command, Some(0));

    let mut transaction = harness.transaction();
    transaction.remove(paint);
    transaction.remove_chunk(ChunkKey::new(1, 0));
    harness.commit(transaction);
}

#[test]
fn removing_a_chunk_and_the_clip_it_names_together_commits() {
    let mut harness = Harness::new();
    let mut transaction = harness.transaction();
    let space = harness.space;
    let clip = transaction.create_clip(ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 8.0, 8.0],
            radii: [0.0; 8],
            shapes: iceblink::ROUND,
        },
    });
    let props = iceblink::ChunkProps {
        clip: Some(clip),
        ..harness.props()
    };
    transaction
        .record(ChunkKey::new(1, 0), props)
        .quad(quad([0.0, 0.0, 8.0, 8.0], [9; 4]));
    harness.commit(transaction);

    let mut transaction = harness.transaction();
    transaction.remove(clip);
    assert_eq!(rejected(&mut harness, transaction), RejectReason::InUse);

    let mut transaction = harness.transaction();
    transaction.remove_chunk(ChunkKey::new(1, 0));
    transaction.remove(clip);
    harness.commit(transaction);
}
