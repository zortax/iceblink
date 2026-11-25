//! Rules on clients, published snapshots, frame watches and stepped runs.

mod workload;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use iceblink::{ChunkKey, OrderEntry, OrderNode};
use iceblink::{ClientEvent, CompositorBudgets, DriverPacing};

use crate::conformance::client::Rig;
use crate::conformance::client::noting::{Noted, Notes, Noting};
use crate::conformance::client::stage::{ClientStage, quads};

pub use self::workload::transcript;

/// An unregistered client loses what it drew, its queue and its transactions, and a new client
/// registers in its place.
pub fn an_unregistered_client_loses_its_content_and_its_queue(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let mut tx = stage.tx();
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        stage.space,
        &[[0.0, 0.0, 20.0, 20.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.driver.commit(tx);
    stage.step();
    let before = stage.frames();

    let tx = stage.tx();
    stage.driver.commit(tx);
    let gone = stage.client;
    stage.driver.unregister_client(gone);
    stage.step();

    let mut events = Vec::new();
    stage.driver.drain_events(gone, &mut events);
    assert!(
        events.is_empty(),
        "the client's queue is gone, found {events:?}"
    );
    let transaction = catch_unwind(AssertUnwindSafe(|| stage.driver.transaction(gone)));
    assert!(
        transaction.is_err(),
        "an unregistered client gets no transaction"
    );
    {
        let log = stage.log();
        assert_eq!(
            log.draws.len(),
            before + 1,
            "removing the content builds a frame"
        );
        assert_eq!(log.draws[before].quads, 0, "the client's quad is gone");
    }

    let (client, _) = stage.driver.register_client();
    assert_ne!(client, gone, "a new client gets a new id");
    let mut tx = stage.driver.transaction(client);
    let root = tx.create_order(OrderNode::default());
    tx.set_root(stage.output, Some(root));
    stage.driver.commit(tx);
    stage.driver.sync();
    let mut events = Vec::new();
    stage.driver.drain_events(client, &mut events);
    assert!(
        matches!(events[..], [ClientEvent::Committed { .. }]),
        "the new client commits, events {events:?}"
    );
}

/// The published snapshots follow the frames built and the schedule, and an output's snapshot
/// holds its renderer's capabilities.
pub fn snapshots_follow_the_frames_and_the_schedule(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    stage.steps(2);
    let stats = stage.driver.stats();
    assert_eq!(
        stats.frames_built,
        stage.frames() as u64,
        "the driver counts the frames drawn"
    );
    assert_eq!(stats.next_due_ns, None, "an idle driver wants no frame");

    let mut tx = stage.tx();
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        stage.space,
        &[[0.0, 0.0, 20.0, 20.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(1, 0)));
    stage.driver.commit(tx);
    stage.driver.sync();
    let due = stage.driver.stats().next_due_ns;
    assert!(
        due.is_some_and(|due| due <= stage.clock().now_ns),
        "a commit wants a frame by the next step, due {due:?}"
    );

    stage.step();
    let status = stage
        .driver
        .output_status(stage.output)
        .expect("the output exists");
    assert_eq!(
        status.frames_built,
        stage.frames() as u64,
        "the output counts its frames"
    );
    assert!(
        status.caps.can_shift,
        "the snapshot holds the renderer's capabilities"
    );
    assert_eq!(
        stage.driver.stats().counters.commit.chunks_recorded,
        1,
        "the snapshot holds the counters of the last commit"
    );
}

/// A frame watch queues a `Drawn` event for every frame drawn, which wakes nothing.
pub fn a_frame_watch_reports_every_frame_drawn(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    stage.driver.watch_frames(stage.client, stage.output, true);
    let before = stage.frames();
    let woken = stage.woken();

    for index in 0..3 {
        let mut tx = stage.tx();
        let key = ChunkKey::new(1 + index, 0);
        let at = 30.0 * index as f32;
        quads(&mut tx, key, stage.space, &[[at, 0.0, 20.0, 20.0]]);
        stage.push(&mut tx, OrderEntry::Chunk(key));
        stage.driver.commit(tx);
        stage.step();
    }

    let drawn: Vec<_> = stage
        .events()
        .into_iter()
        .filter_map(|event| match event {
            ClientEvent::Drawn { frame, .. } => Some(frame),
            _ => None,
        })
        .collect();
    let log = stage.log();
    let frames: Vec<_> = log.draws[before..]
        .iter()
        .map(|frame| frame.draw.frame)
        .collect();
    assert_eq!(
        drawn, frames,
        "every frame drawn is reported once, in order"
    );
    assert_eq!(stage.woken(), woken, "frame reports wake nothing");
}

/// Two runs of one workload on stepped drivers draw the same frames and queue the same events.
pub fn a_stepped_run_reproduces_its_frames_and_events(rig: &Rig<'_>) {
    let first = transcript(rig);
    let second = transcript(rig);
    assert_eq!(first, second, "two stepped runs of one workload agree");
}

/// Verification and budgets are applied to the compositor in the order they were queued among
/// commits.
pub fn verification_and_budgets_apply_in_queue_order(rig: &Rig<'_>) {
    let notes = Notes::default();
    let compositor = Box::new(Noting {
        inner: (rig.compositor)(),
        notes: Arc::clone(&notes),
    });
    let mut driver = (rig.client)(compositor);
    let (client, _) = driver.register_client();
    let budgets = CompositorBudgets {
        atlas_bytes: Some(1 << 20),
    };

    driver.set_verification(true);
    let tx = driver.transaction(client);
    driver.commit(tx);
    driver.set_budgets(budgets);
    driver.set_verification(false);
    driver.sync();

    let noted = notes.lock().unwrap().clone();
    assert_eq!(
        noted,
        [
            Noted::Verification(true),
            Noted::Commit,
            Noted::Budgets(budgets),
            Noted::Verification(false)
        ],
        "the calls reach the compositor in the order they were queued"
    );
}

/// A stepped driver reports that it builds frames only in `step`.
pub fn a_stepped_driver_reports_its_pacing(rig: &Rig<'_>) {
    let driver = (rig.client)((rig.compositor)());
    assert_eq!(
        driver.pacing(),
        DriverPacing::Stepped,
        "the client must step the driver"
    );
}

/// The compositor's memory is current once the driver answered a `sync`.
pub fn memory_is_current_after_a_sync(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    stage.driver.sync();
    let before = stage.driver.memory();
    let mut tx = stage.tx();
    let key = ChunkKey::new(1, 0);
    quads(&mut tx, key, stage.space, &[[0.0, 0.0, 40.0, 40.0]]);
    stage.push(&mut tx, OrderEntry::Chunk(key));
    stage.driver.commit(tx);
    stage.driver.sync();

    assert!(
        stage.driver.memory().chunk_bytes > before.chunk_bytes,
        "a sync answers with the memory of the chunk the commit before it recorded"
    );
}
