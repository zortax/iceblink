//! The commit counters a precise compositor keeps.

use iceblink::{Color, Matrix4, OrderEntry, Paint};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, props, record_quads};
use crate::conformance::scene::stage::Stage;

const RED: [u8; 4] = [255, 0, 0, 255];

/// A precise compositor counts what each commit recorded, removed, moved and wrote, exactly.
pub fn commit_counters(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    if !stage.precise("commit_counters") {
        return;
    }

    let mut tx = stage.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 0.0, 0.0, 1.0])));
    let space = tx.create_transform(identity(Some(stage.space)));
    record_quads(
        &mut tx,
        key(1),
        props(space),
        &[([0.0, 0.0, 8.0, 8.0], RED)],
    );
    record_quads(
        &mut tx,
        key(2),
        stage.props(),
        &[([20.0, 0.0, 8.0, 8.0], RED)],
    );
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.append(&mut tx, OrderEntry::Chunk(key(2)));
    let counters = stage.commit(tx).counters;
    assert_eq!(counters.chunks_recorded, 2, "chunks recorded");
    assert_eq!(counters.keys_spliced, 2, "keys spliced");
    assert_eq!(
        counters.property_writes, 0,
        "creates are not property writes"
    );
    assert!(counters.bytes_recorded > 0, "bytes recorded");

    let mut tx = stage.tx();
    tx.set_offset(key(1), [2.0, 0.0]);
    tx.set_transform(space, Matrix4::translation(1.0, 0.0, 0.0));
    tx.set_paint(paint, Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    tx.remove_chunk(key(2));
    let counters = stage.commit(tx).counters;
    assert_eq!(counters.offsets_written, 1, "offsets written");
    assert_eq!(counters.property_writes, 1, "property writes");
    assert_eq!(counters.paint_writes, 1, "paint writes");
    assert_eq!(counters.chunks_removed, 1, "chunks removed");
    assert_eq!(counters.chunks_recorded, 0, "nothing recorded");
}
