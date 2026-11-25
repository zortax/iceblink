//! Scrolling: content follows the offset, a shift is valid wherever one is made, a precise
//! compositor shifts where the rules allow it, and fixed and sticky nodes hold their place.

use iceblink::{
    Anchor, BufferAge, Built, ClipId, ClipNode, ClipShape, OrderEntry, ROUND, ScrollId, ScrollNode,
    StickyAnchor, TransformId, TransformNode,
};

use crate::conformance::scene::MakeCompositor;
use crate::conformance::scene::build::{identity, key, pixels, props, record_quads};
use crate::conformance::scene::stage::{Stage, covers, presented};

const PORT: [f32; 4] = [16.0, 16.0, 64.0, 96.0];
const BACKING: [u8; 4] = [250, 250, 250, 255];
const ROWS: [[u8; 4]; 3] = [[200, 0, 0, 255], [0, 150, 0, 255], [0, 0, 200, 255]];

/// A scroll port over `PORT` in the root space, a clip of the port, and the space of its content.
struct Port {
    scroll: ScrollId,
    content: TransformId,
    clip: ClipId,
}

/// Builds a port backed by `backing`, with three rows of content under it.
fn port(stage: &mut Stage, backing: [u8; 4]) -> Port {
    let mut tx = stage.tx();
    let scroll = tx.create_scroll(ScrollNode {
        space: stage.space,
        port: PORT,
        content: [64.0, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(stage.space))
    });
    let clip = tx.create_clip(ClipNode {
        parent: None,
        space: stage.space,
        shape: ClipShape::RoundedRect {
            rect: PORT,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    record_quads(&mut tx, key(1), stage.props(), &[(PORT, backing)]);
    let mut clipped = props(content);
    clipped.clip = Some(clip);
    let rows: Vec<([f32; 4], [u8; 4])> = ROWS
        .iter()
        .enumerate()
        .map(|(index, &color)| ([20.0, 30.0 + index as f32 * 40.0, 50.0, 20.0], color))
        .collect();
    record_quads(&mut tx, key(2), clipped, &rows);
    stage.append(&mut tx, OrderEntry::Chunk(key(1)));
    stage.append(&mut tx, OrderEntry::Chunk(key(2)));
    stage.commit(tx);
    stage.draw();
    Port {
        scroll,
        content,
        clip,
    }
}

/// Scrolls `port` to `offset` and builds the frame.
fn scroll_to(stage: &mut Stage, port: &Port, offset: f32) {
    let mut tx = stage.tx();
    tx.scroll_to(port.scroll, [0.0, offset]);
    stage.commit(tx);
    assert_eq!(stage.build(), Built::Frame, "a scroll builds a frame");
}

/// Asserts that every shift of the last frame moves pixels of the port as far as the content moved
/// from offset `before` to `after`, and that its damage and shifts together redraw the rows at
/// both offsets.
#[track_caller]
fn sound(stage: &Stage, before: f32, after: f32) {
    let port = pixels(PORT);
    for shift in &stage.frame.shifts {
        assert!(
            port.contains_rect(shift.region),
            "a shift moves only pixels of the port: {:?}",
            shift.region
        );
        assert_eq!(
            shift.by,
            [0, (before - after) as i32],
            "a shift moves the pixels as far as the content moved"
        );
    }
    for index in 0..ROWS.len() {
        for offset in [before, after] {
            let row = [20.0, 30.0 + index as f32 * 40.0 - offset, 50.0, 20.0];
            let Some(seen) = pixels(row).intersection(port) else {
                continue;
            };
            let shifted = stage
                .frame
                .shifts
                .iter()
                .any(|shift| shift.region.contains_rect(seen));
            assert!(
                shifted || covers(stage.damage(), seen),
                "a row at {seen:?} is neither shifted nor damaged"
            );
        }
    }
}

/// Content under a scroll node moves by the offset, and a frame either shifts the port's pixels
/// or damages them.
pub fn scrolling(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let port = port(&mut stage, BACKING);
    scroll_to(&mut stage, &port, 10.0);
    sound(&stage, 0.0, 10.0);
    stage.done(presented());

    let picture = stage.redraw();
    let row = picture.quad(ROWS[0]);
    assert_eq!(
        row.bounds,
        [20.0, 20.0, 50.0, 20.0],
        "a row moves up by the offset"
    );
    assert!(
        row.clip.is_some(),
        "scrolled content is drawn through the port's clip"
    );
}

/// A precise compositor answers a whole-pixel scroll of a port backed by an opaque colour with a
/// shift, and nobody shifts where the backing is translucent, the move is fractional, or the
/// target holds nothing.
pub fn shifts(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let port = port(&mut stage, BACKING);
    scroll_to(&mut stage, &port, 8.0);
    sound(&stage, 0.0, 8.0);
    if stage.precise("shifts") {
        assert_eq!(
            stage.frame.shifts.len(),
            1,
            "a precise compositor shifts an opaque port by a whole-pixel scroll"
        );
        assert!(
            !covers(stage.damage(), pixels(PORT)),
            "the shift saves redrawing the whole port"
        );
    }
    stage.done(presented());

    scroll_to(&mut stage, &port, 8.5);
    assert!(
        stage.frame.shifts.is_empty(),
        "a fractional move is not a shift of whole pixels"
    );
    sound(&stage, 8.0, 8.5);
    stage.done(presented());

    let mut tx = stage.tx();
    tx.scroll_to(port.scroll, [0.0, 20.0]);
    stage.commit(tx);
    assert_eq!(stage.build_aged(BufferAge::Unknown), Built::Frame);
    assert!(
        stage.frame.shifts.is_empty(),
        "a frame for an image of unknown contents has no shift"
    );
    stage.done(presented());

    let mut translucent = Stage::full(make);
    let port = translucent_port(&mut translucent);
    scroll_to(&mut translucent, &port, 8.0);
    assert!(
        translucent.frame.shifts.is_empty(),
        "a port without an opaque backing is not shifted"
    );
    sound(&translucent, 0.0, 8.0);
}

/// A port backed by a translucent colour.
fn translucent_port(stage: &mut Stage) -> Port {
    port(stage, [120, 120, 120, 128])
}

/// A fixed node holds still while the scroll above it moves, a sticky node is held inside the
/// port by its inset, and a fixed node under a sticky node keeps the stick.
pub fn anchors(make: MakeCompositor<'_>) {
    let mut stage = Stage::full(make);
    let port = port(&mut stage, BACKING);
    let fixed_color = [255, 128, 0, 255];
    let sticky_color = [128, 0, 255, 255];
    let pinned_color = [0, 128, 255, 255];
    let header = [20.0, 20.0, 50.0, 8.0];

    let mut tx = stage.tx();
    let fixed = tx.create_transform(TransformNode {
        anchor: Anchor::Fixed,
        ..identity(Some(port.content))
    });
    let sticky = tx.create_transform(TransformNode {
        anchor: Anchor::Sticky(StickyAnchor {
            scroll: port.scroll,
            rect: header,
            limit: [16.0, 0.0, 64.0, 400.0],
            inset: [0.0, f32::NAN, f32::NAN, f32::NAN],
        }),
        ..identity(Some(port.content))
    });
    let mut clipped = props(fixed);
    clipped.clip = Some(port.clip);
    record_quads(
        &mut tx,
        key(3),
        clipped,
        &[([60.0, 90.0, 10.0, 10.0], fixed_color)],
    );
    let mut clipped = props(sticky);
    clipped.clip = Some(port.clip);
    record_quads(&mut tx, key(4), clipped, &[(header, sticky_color)]);
    let pinned = tx.create_transform(TransformNode {
        anchor: Anchor::Fixed,
        ..identity(Some(sticky))
    });
    record_quads(
        &mut tx,
        key(5),
        props(pinned),
        &[([90.0, 20.0, 8.0, 8.0], pinned_color)],
    );
    stage.append(&mut tx, OrderEntry::Chunk(key(3)));
    stage.append(&mut tx, OrderEntry::Chunk(key(4)));
    stage.append(&mut tx, OrderEntry::Chunk(key(5)));
    stage.commit(tx);
    stage.draw();

    scroll_to(&mut stage, &port, 30.0);
    stage.done(presented());
    let picture = stage.redraw();
    assert_eq!(
        picture.quad(fixed_color).bounds,
        [60.0, 90.0, 10.0, 10.0],
        "a fixed node holds still while its scroll moves"
    );
    assert_eq!(
        picture.quad(sticky_color).bounds,
        [20.0, 16.0, 50.0, 8.0],
        "a sticky node with a top inset of zero stops at the port's top edge"
    );
    assert_eq!(
        picture.quad(pinned_color).bounds,
        [90.0, 46.0, 8.0, 8.0],
        "a fixed node under a sticky one drops the scroll offset and keeps the stick of 26"
    );
}
