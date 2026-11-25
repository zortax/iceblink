//! A client's transactions reuse the buffers of its applied commits.

mod common;

use iceblink::CompositorClient;
use iceblink::{ChunkKey, Color, OrderEntry, Paint, PaintKind};
use iceblink_testing::{CountingAlloc, allocations};

use crate::common::thread::probe::Probe;
use crate::common::thread::scene::{List, PERIOD_NS, commit, props, quad, spawn};
use crate::common::thread::watched::Watched;

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

const CHUNK: ChunkKey = ChunkKey::new(7, 0);

#[test]
fn a_recording_into_the_last_commits_buffers_allocates_nothing() {
    let probe = Probe::new();
    let (driver, mut handle) = spawn();
    let mut list = List::open(&mut handle, Watched::factory(&probe, PERIOD_NS));
    let mut tx = handle.transaction(list.client);
    let paint = tx.create_paint(Paint::Solid(Color([0.2, 0.4, 0.8, 1.0])));
    list.push(&mut tx, OrderEntry::Chunk(CHUNK));
    commit(&mut handle, tx);

    let rects: Vec<[f32; 4]> = (0..64)
        .map(|row| [0.0, row as f32 * 3.0, 50.0, 2.0])
        .collect();
    let mut counted = Vec::new();
    for _ in 0..12 {
        let mut recorded = None;
        let count = allocations(|| {
            let mut tx = handle.transaction(list.client);
            {
                let mut chunk = tx.record(CHUNK, props(list.space));
                let fill = chunk.paint_ref(PaintKind::SOLID, paint);
                for &rect in &rects {
                    chunk.quad(quad(rect, fill));
                }
            }
            recorded = Some(tx);
        });
        counted.push(count);
        commit(&mut handle, recorded.expect("the transaction was recorded"));
    }
    assert_eq!(
        counted[2..],
        [0; 10],
        "recordings after the first reuse the buffers, allocations {counted:?}"
    );
    driver.shutdown();
}
