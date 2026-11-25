//! Thumb nodes: a node anchored to a scroll node moves along its track with the offset.

use iceblink::{
    Anchor, Built, OrderEntry, RejectReason, ScrollId, ScrollNode, ThumbAnchor, TransformNode,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, pixels, props, record_quads};
use crate::conformance::scene::stage::{Stage, covers, presented};

const PORT: [f32; 4] = [16.0, 16.0, 64.0, 96.0];
const THUMB: [u8; 4] = [40, 40, 200, 255];

/// The thumb at offset zero, beside the port.
const AT_START: [f32; 4] = [84.0, 16.0, 6.0, 36.0];

/// How far the thumb moves over the whole range.
const TRACK: f32 = 60.0;

/// Builds a port whose largest offset is 304, and a thumb beside it with a track of `TRACK`.
fn thumb(stage: &mut Stage) -> ScrollId {
    let mut tx = stage.tx();
    let scroll = tx.create_scroll(ScrollNode {
        space: stage.space,
        port: PORT,
        content: [64.0, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let node = tx.create_transform(TransformNode {
        anchor: Anchor::Thumb(ThumbAnchor {
            scroll,
            track: [0.0, TRACK],
        }),
        ..identity(Some(stage.space))
    });
    record_quads(&mut tx, key(1), props(node), &[(AT_START, THUMB)]);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.commit(tx);
    stage.draw();
    scroll
}

/// Scrolls to `offset`, builds the frame, and asserts that its damage holds the thumb where it
/// was and where it is.
#[track_caller]
fn scroll_to(stage: &mut Stage, scroll: ScrollId, offset: f32, before: f32, after: f32) {
    let mut tx = stage.tx();
    tx.scroll_to(scroll, [0.0, offset]);
    stage.commit(tx);
    assert_eq!(stage.build(), Built::Frame, "a scroll builds a frame");
    for top in [before, after] {
        let [x, _, width, height] = AT_START;
        assert!(
            covers(stage.damage(), pixels([x, top, width, height])),
            "a moving thumb is damaged where it was and where it is, at {top}"
        );
    }
    stage.done(presented());
}

/// A thumb node sits at the start of its track at offset zero, at the end at the largest offset,
/// and in proportion between, and a scroll node without a range leaves it at the start.
///
/// A scroll node a thumb follows stays in use, and a thumb's track is finite.
pub fn thumbs(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let scroll = thumb(&mut stage);
    let top = AT_START[1];
    assert_eq!(stage.redraw().quad(THUMB).bounds, AT_START);

    scroll_to(&mut stage, scroll, 152.0, top, top + TRACK / 2.0);
    assert_eq!(
        stage.redraw().quad(THUMB).bounds[1],
        top + TRACK / 2.0,
        "half the range moves the thumb half its track"
    );

    scroll_to(&mut stage, scroll, 304.0, top + TRACK / 2.0, top + TRACK);
    assert_eq!(
        stage.redraw().quad(THUMB).bounds[1],
        top + TRACK,
        "the largest offset puts the thumb at the end of its track"
    );

    let mut tx = stage.tx();
    tx.set_scroll_node(
        scroll,
        ScrollNode {
            space: stage.space,
            port: PORT,
            content: [64.0, 90.0],
            offset: [0.0, 0.0],
            behaviour: None,
        },
    );
    stage.commit(tx);
    assert_eq!(
        stage.redraw().quad(THUMB).bounds,
        AT_START,
        "a scroll node with nothing to scroll leaves the thumb at the start"
    );

    let mut tx = stage.tx();
    tx.remove(scroll);
    let found = stage.try_commit(tx).map_err(|error| error.reason);
    assert_eq!(
        found,
        Err(RejectReason::InUse),
        "a scroll node a thumb follows stays in use"
    );

    let mut tx = stage.tx();
    tx.create_transform(TransformNode {
        anchor: Anchor::Thumb(ThumbAnchor {
            scroll,
            track: [0.0, f32::NAN],
        }),
        ..identity(Some(stage.space))
    });
    let found = stage.try_commit(tx).map_err(|error| error.reason);
    assert_eq!(
        found,
        Err(RejectReason::InvalidItem),
        "a thumb's track is finite"
    );
}
