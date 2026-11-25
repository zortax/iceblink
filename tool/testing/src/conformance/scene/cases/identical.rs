//! Identical replace and the per-position diff of a recording, for a precise compositor.

use iceblink::Built;

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{key, pixels, record_quads};
use crate::conformance::scene::stage::{Stage, area, covers};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// Three quads far apart in one chunk.
fn three(middle: [u8; 4]) -> [([f32; 4], [u8; 4]); 3] {
    [
        ([4.0, 4.0, 16.0, 16.0], RED),
        ([56.0, 56.0, 16.0, 16.0], middle),
        ([100.0, 100.0, 16.0, 16.0], BLUE),
    ]
}

/// Recording a chunk again with identical items adds no damage, and recording it with one item
/// changed damages only that item.
pub fn identical_replace(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    if !stage.precise("identical_replace") {
        return;
    }
    stage.add_quads(key(1), &three(GREEN));
    stage.draw();

    let mut tx = stage.tx();
    record_quads(&mut tx, key(1), stage.props(), &three(GREEN));
    let report = stage.commit(tx);
    assert_eq!(
        report.counters.chunks_identical, 1,
        "the recording counts as identical"
    );
    assert!(
        matches!(stage.draw(), Built::Idle { .. }),
        "an identical recording adds no damage, but a frame was built with {:?}",
        stage.damage()
    );

    let mut tx = stage.tx();
    record_quads(&mut tx, key(1), stage.props(), &three([255, 255, 0, 255]));
    stage.commit(tx);
    assert_eq!(stage.draw(), Built::Frame);
    let middle = pixels([56.0, 56.0, 16.0, 16.0]);
    assert!(
        covers(stage.damage(), middle),
        "the changed item is damaged"
    );
    assert_eq!(
        area(stage.damage()),
        i64::from(middle.size.width) * i64::from(middle.size.height),
        "only the changed position is damaged: {:?}",
        stage.damage()
    );
}
