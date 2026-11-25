//! World matrices under scroll offsets, fixed anchors and sticky anchors.

mod common;

use iceblink::{
    Anchor, ChunkKey, ChunkProps, Matrix4, OrderEntry, ScrollNode, StickyAnchor, TransformId,
    TransformNode,
};

use crate::common::naive::{Harness, quad, root_node};

/// Records one quad under `space` and returns the world matrix its frame space slot holds.
fn world_of(harness: &mut Harness, space: TransformId, key: ChunkKey) -> Matrix4 {
    let mut transaction = harness.transaction();
    let props = ChunkProps {
        space,
        ..harness.props()
    };
    transaction
        .record(key, props)
        .quad(quad([0.0, 0.0, 4.0, 60.0], [9; 4]));
    harness.append(&mut transaction, OrderEntry::Chunk(key));
    harness.commit(transaction);

    assert!(harness.frame());
    let frame = &harness.frame;
    let slot = frame
        .lanes
        .quads
        .items
        .last()
        .expect("the quad is drawn")
        .space;
    frame.tables.spaces.all[slot.0 as usize]
}

/// Returns the translation part of `matrix`.
fn translation(matrix: Matrix4) -> [f32; 2] {
    [matrix.columns[3][0], matrix.columns[3][1]]
}

#[test]
fn scroll_offsets_move_scrolled_nodes_and_not_fixed_ones_and_hold_sticky_ones_by_their_insets() {
    let mut harness = Harness::new();
    let root = harness.space;
    let mut transaction = harness.transaction();
    let scroll = transaction.create_scroll(ScrollNode {
        space: root,
        port: [0.0, 0.0, 64.0, 48.0],
        content: [64.0, 100.0],
        offset: [5.0, 30.0],
        behaviour: None,
    });
    let scrolled = transaction.create_transform(TransformNode {
        parent: Some(root),
        scroll: Some(scroll),
        ..root_node()
    });
    let fixed = transaction.create_transform(TransformNode {
        parent: Some(scrolled),
        anchor: Anchor::Fixed,
        local: Matrix4::translation(2.0, 3.0, 0.0),
        ..root_node()
    });
    let sticky = transaction.create_transform(TransformNode {
        parent: Some(scrolled),
        anchor: Anchor::Sticky(StickyAnchor {
            scroll,
            rect: [0.0, 10.0, 64.0, 8.0],
            limit: [0.0, 0.0, 64.0, 100.0],
            inset: [2.0, f32::NAN, f32::NAN, f32::NAN],
        }),
        ..root_node()
    });
    harness.commit(transaction);

    // Horizontally the content is no wider than the port, so the offset clamps to zero.
    let world = world_of(&mut harness, scrolled, ChunkKey::new(1, 0));
    assert_eq!(translation(world), [0.0, -30.0]);

    let world = world_of(&mut harness, fixed, ChunkKey::new(2, 0));
    assert_eq!(translation(world), [2.0, 3.0]);

    // The sticky rectangle sits 20 above the port's top, so it moves down to 2 below it.
    let world = world_of(&mut harness, sticky, ChunkKey::new(3, 0));
    assert_eq!(translation(world), [0.0, -8.0]);
}
