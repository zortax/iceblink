//! Rules on configuring an output whose frames started.

use iceblink::{ChunkKey, OrderEntry, OutputConfig};

use crate::conformance::client::Rig;
use crate::conformance::client::stage::{ClientStage, quads};

/// A configuration takes effect with the next commit or `request_frame` queued after it, so a
/// frame at a new size shows every commit queued after the `configure_output` that set it.
pub fn a_new_size_is_drawn_only_with_the_commits_queued_after_it(rig: &Rig<'_>) {
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

    stage
        .driver
        .configure_output(stage.output, sized([300, 200]));
    stage.steps(2);
    assert!(
        stage.log().draws[before..]
            .iter()
            .all(|frame| frame.size == [200, 200]),
        "no frame is drawn at the new size before a commit queued after the configure"
    );

    let mut tx = stage.tx();
    quads(
        &mut tx,
        ChunkKey::new(2, 0),
        stage.space,
        &[[280.0, 0.0, 20.0, 20.0]],
    );
    stage.push(&mut tx, OrderEntry::Chunk(ChunkKey::new(2, 0)));
    stage.driver.commit(tx);
    stage.step();
    let shown: Vec<_> = stage.log().draws[before..]
        .iter()
        .map(|frame| (frame.size, frame.quads))
        .collect();
    assert!(
        matches!(shown[..], [([300, 200], quads)] if quads > 0),
        "the commit's frame is drawn at the new size and shows the commit, frames {shown:?}"
    );

    let before = stage.frames();
    stage
        .driver
        .configure_output(stage.output, sized([300, 240]));
    stage.driver.request_frame(stage.output);
    stage.step();
    let sizes: Vec<_> = stage.log().draws[before..]
        .iter()
        .map(|frame| frame.size)
        .collect();
    assert_eq!(
        sizes,
        [[300, 240]],
        "a frame requested after the configure is drawn at the new size"
    );
}

fn sized(size: [u32; 2]) -> OutputConfig {
    OutputConfig {
        size,
        scale: 1.0,
        opaque: true,
    }
}
