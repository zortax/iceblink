//! Building transactions from the buffers of earlier ones.

mod common;

use std::sync::Arc;

use iceblink::{ChunkKey, MarkPayload, OrderNode, Transaction};

#[test]
fn a_recycled_transaction_is_empty_and_keeps_its_capacity() {
    let ids = common::ids();
    let mut transaction = Transaction::new(ids.clone());
    let props = common::props(&mut transaction);
    let order = transaction.create_order(OrderNode::default());
    transaction.splice(order, 0, 0, &[]);
    let payload = Arc::new(MarkPayload::default());
    let mut recorder = transaction.record(ChunkKey::new(1, 0), props);
    for x in 0..64 {
        recorder.quad(common::quad(x as f32));
    }
    recorder.payload(payload.clone());
    let parts = transaction.into_parts();
    let commands = parts.commands.capacity();
    let quads = parts.chunks[0].quads.as_ptr();

    let mut recycled = Transaction::from_recycled(ids, parts);
    assert!(recycled.is_empty());
    assert_eq!(Arc::strong_count(&payload), 1);

    let props = common::props(&mut recycled);
    recycled
        .record(ChunkKey::new(2, 0), props)
        .quad(common::quad(0.0));
    let parts = recycled.into_parts();

    assert_eq!(parts.commands.capacity(), commands);
    assert_eq!(parts.minted.len(), 1);
    assert!(parts.entries.is_empty());
    assert_eq!(parts.chunks.len(), 1);
    assert_eq!(parts.chunks[0].key, ChunkKey::new(2, 0));
    assert_eq!(parts.chunks[0].quads.len(), 1);
    assert_eq!(parts.chunks[0].quads.as_ptr(), quads);
    assert!(parts.chunks[0].payloads.is_empty());
    assert!(parts.spare.is_empty());
}

#[test]
fn chunks_a_recycled_transaction_does_not_record_into_stay_spare() {
    let ids = common::ids();
    let mut transaction = Transaction::new(ids.clone());
    let props = common::props(&mut transaction);
    for owner in 0..3 {
        transaction
            .record(ChunkKey::new(owner, 0), props)
            .quad(common::quad(0.0));
    }

    let recycled = Transaction::from_recycled(ids, transaction.into_parts());
    let parts = recycled.into_parts();

    assert!(parts.chunks.is_empty());
    assert_eq!(parts.spare.len(), 3);
    assert!(
        parts
            .spare
            .iter()
            .all(|chunk| chunk.ops.is_empty() && chunk.quads.is_empty())
    );
}
