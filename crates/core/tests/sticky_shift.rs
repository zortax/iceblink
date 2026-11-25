//! A scroll of content drawn through a clip in a sticky space, drawn into a kept target on a
//! device, against the same pictures redrawn whole.

mod common;

use iceblink::{
    Anchor, ChunkKey, ClipId, ClipNode, ClipShape, OrderEntry, PaintKind, ROUND, ScrollId,
    ScrollNode, StickyAnchor, TransformId, TransformNode,
};
use iceblink_testing::gpu::device_lock;

use common::twin::Twin;
use common::{identity, props, quad};

/// Returns a clip node over `rect` in `space`, without radii.
fn square(space: TransformId, rect: [f32; 4], parent: Option<ClipId>) -> ClipNode {
    ClipNode {
        parent,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

/// Builds a white backing, a scroll port over the top left 100 pixels holding ten rows, and a
/// header that sticks to the top of the port once scrolled 40 pixels, drawn through a clip in
/// its own space. Returns the scroll node.
fn scene(twin: &mut Twin) -> ScrollId {
    twin.commit(&mut (), |tx, world, _| {
        let space = world.space;
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        world.boxes(
            tx,
            ChunkKey::new(1, 0),
            white,
            &[[0.0, 0.0, 128.0, 128.0]],
            true,
        );
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: [0.0, 0.0, 100.0, 100.0],
            content: [100.0, 400.0],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(scroll),
            ..identity(Some(space))
        });
        let sticky = tx.create_transform(TransformNode {
            anchor: Anchor::Sticky(StickyAnchor {
                scroll,
                rect: [0.0, 40.0, 100.0, 12.0],
                limit: [0.0, 0.0, 100.0, 400.0],
                inset: [0.0, f32::NAN, f32::NAN, f32::NAN],
            }),
            ..identity(Some(content))
        });
        let port = tx.create_clip(square(space, [0.0, 0.0, 100.0, 100.0], None));
        let header_clip = tx.create_clip(square(sticky, [2.0, 40.0, 90.0, 12.0], Some(port)));

        let rows = world.solid(tx, [0.2, 0.3, 0.7, 1.0]);
        let header = world.solid(tx, [0.8, 0.3, 0.1, 1.0]);
        for row in 0..10 {
            let key = ChunkKey::new(2, row);
            let mut row_props = props(content);
            row_props.clip = Some(port);
            let mut chunk = tx.record(key, row_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, rows);
            chunk.quad(quad([4.0, 60.0 + row as f32 * 24.0, 80.0, 16.0], fill));
            world.append(tx, OrderEntry::Chunk(key));
        }
        let key = ChunkKey::new(3, 0);
        let mut header_props = props(sticky);
        header_props.clip = Some(header_clip);
        let mut chunk = tx.record(key, header_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, header);
        chunk.quad(quad([0.0, 38.0, 100.0, 16.0], fill));
        world.append(tx, OrderEntry::Chunk(key));
        scroll
    })
}

#[test]
fn a_header_clipped_in_its_sticky_space_shifts_until_it_sticks() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let scroll = scene(&mut twin);
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    for step in 1..=8 {
        let offset = step as f32 * 10.0;
        twin.commit(&mut (), |tx, _, _| tx.scroll_to(scroll, [0.0, offset]));
        let found = twin.draw();
        assert_eq!(found.pixels, 0, "the scroll to {offset} differs: {found:?}");
        if offset <= 30.0 {
            assert_eq!(
                twin.kept.frame.shifts.len(),
                1,
                "the scroll to {offset} moves the header with the rows"
            );
        }
    }
}
