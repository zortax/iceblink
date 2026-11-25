//! Rules on queued commits: their answers and the frame that shows them.

use iceblink::ClientEvent;
use iceblink::{ChunkKey, OrderEntry, RejectReason};

use crate::conformance::client::Rig;
use crate::conformance::client::stage::{ClientStage, quads};

/// Every commit is answered by one event in the order commits apply, and only a rejection wakes
/// the client.
pub fn commits_are_answered_in_order_and_only_rejections_wake(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let woken = stage.woken();

    let mut tx = stage.tx();
    quads(
        &mut tx,
        ChunkKey::new(1, 0),
        stage.space,
        &[[0.0, 0.0, 20.0, 20.0]],
    );
    let first = stage.driver.commit(tx);
    let mut tx = stage.tx();
    tx.set_offset(ChunkKey::new(9, 0), [1.0, 0.0]);
    let rejected = stage.driver.commit(tx);
    let tx = stage.tx();
    let last = stage.driver.commit(tx);
    stage.driver.sync();

    let answers: Vec<_> = stage
        .events()
        .into_iter()
        .filter_map(|event| match event {
            ClientEvent::Committed { commit, .. } => Some((commit, None)),
            ClientEvent::Rejected { commit, error } => Some((commit, Some(error.reason))),
            _ => None,
        })
        .collect();
    assert_eq!(
        answers,
        [
            (first, None),
            (rejected, Some(RejectReason::UnknownChunk)),
            (last, None)
        ],
        "each commit is answered once, in order"
    );
    assert_eq!(
        stage.woken(),
        woken + 1,
        "only the rejection wakes the client"
    );
}

/// A frame shows every commit queued before its step, and none queued after it.
pub fn a_frame_shows_the_commits_queued_before_its_step(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    let before = stage.frames();

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
    assert_eq!(stage.frames(), before, "no frame is built before the step");

    stage.step();
    let mut tx = stage.tx();
    quads(
        &mut tx,
        ChunkKey::new(2, 0),
        stage.space,
        &[[100.0, 100.0, 20.0, 20.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(2, 0)));
    stage.driver.commit(tx);
    stage.driver.sync();

    stage.step();

    let log = stage.log();
    let shown: Vec<usize> = log.draws[before..]
        .iter()
        .map(|frame| frame.quads)
        .collect();
    assert_eq!(
        shown.first(),
        Some(&1),
        "the step's frame draws the quad committed before it and not the one after, frames {shown:?}"
    );
    assert!(
        shown.len() == 2 && shown[1] >= 1,
        "the next step's frame draws the quad committed before it, frames {shown:?}"
    );
}

/// A commit that changes nothing that moves on its own builds one frame, after which the driver
/// wants no frame.
pub fn a_commit_builds_one_frame_and_the_driver_idles(rig: &Rig<'_>) {
    let mut stage = ClientStage::new(rig);
    stage.steps(2);
    let before = stage.frames();

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
    let idle = stage.driver.stats().next_due_ns;
    stage.steps(3);

    assert_eq!(
        stage.frames(),
        before + 1,
        "the commit builds exactly one frame"
    );
    assert_eq!(idle, None, "the driver wants no frame after it");
}
