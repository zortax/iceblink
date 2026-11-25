//! Commits that leave a chunk's pixels alone measure and look up nothing for it, as the work
//! counters show.

mod common;

use iceblink::Compositor;
use iceblink_testing::capture::CaptureRenderer;
use std::sync::Arc;

use iceblink::{
    BlendMode, ChunkKey, ClipId, ClipNode, ClipShape, EffectNode, OrderEntry, OrderNode, PaintKind,
    ROUND, ScrollNode, TransformNode,
};

use common::slice::Slice;
use common::text::run;
use common::{FULL, identity, props, quad, stat};

type Captured = Slice<CaptureRenderer>;

/// Returns a slice past its first frame.
fn slice() -> Captured {
    let mut slice = Slice::new(CaptureRenderer::with_capabilities(FULL));
    slice.draw();
    slice
}

/// Returns how much the work counter `name` grows while `then` runs.
fn work(slice: &mut Captured, name: &str, then: impl FnOnce(&mut Captured)) -> u64 {
    let before = stat(&slice.compositor, name);
    then(slice);
    stat(&slice.compositor, name) - before
}

/// Returns a clip node over `rect` in the slice's root space.
fn square(slice: &Captured, rect: [f32; 4]) -> ClipNode {
    ClipNode {
        parent: None,
        space: slice.world.space,
        shape: ClipShape::RoundedRect {
            rect,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

/// Records ten rows of 10 pixels every 12 pixels, each a chunk of its own drawn through a clip
/// over the top 100 pixels, and returns the clip.
fn rows(slice: &mut Captured) -> ClipId {
    rows_of(slice, 10)
}

/// Records `count` rows as `rows` does, and returns the clip.
fn rows_of(slice: &mut Captured, count: u32) -> ClipId {
    let node = square(slice, [0.0, 0.0, 128.0, 100.0]);
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let clip = tx.create_clip(node);
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    for row in 0..count {
        let key = ChunkKey::new(1, row);
        let mut row_props = props(world.space);
        row_props.clip = Some(clip);
        let mut chunk = tx.record(key, row_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([4.0, row as f32 * 12.0, 80.0, 10.0], fill));
        world.append(&mut tx, OrderEntry::Chunk(key));
    }
    slice.compositor.commit(tx).expect("the rows commit");
    assert!(slice.draw());
    clip
}

#[test]
fn a_clip_edge_that_moves_measures_only_the_rows_it_cuts_differently() {
    let mut slice = slice();
    let clip = rows(&mut slice);

    // Row 8 spans 96 to 106 and is the only one that meets the edge between 95 and 100. Row 9
    // lies below both edges.
    let node = square(&slice, [0.0, 0.0, 128.0, 95.0]);
    let measured = work(&mut slice, "work.inks_computed", |slice| {
        let mut tx = slice.tx();
        tx.set_clip(clip, node);
        slice.compositor.commit(tx).expect("the clip commits");
        assert!(slice.draw());
    });
    assert_eq!(measured, 1, "only the row the edge crosses is measured");

    // Growing the clip past rows it cut away whole measures them again.
    let node = square(&slice, [0.0, 0.0, 128.0, 128.0]);
    let measured = work(&mut slice, "work.inks_computed", |slice| {
        let mut tx = slice.tx();
        tx.set_clip(clip, node);
        slice.compositor.commit(tx).expect("the clip commits");
        assert!(slice.draw());
    });
    assert_eq!(measured, 2, "rows 8 and 9, which the clip now admits");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_clip_edge_that_moves_checks_only_the_users_it_cuts_differently() {
    let mut slice = slice();
    let clip = rows_of(&mut slice, 40);
    let write = |slice: &mut Captured, bottom: f32| {
        let node = square(slice, [0.0, 0.0, 128.0, bottom]);
        work(slice, "work.clip_users_checked", |slice| {
            let mut tx = slice.tx();
            tx.set_clip(clip, node);
            slice.compositor.commit(tx).expect("the clip commits");
            assert!(slice.draw());
        })
    };
    assert_eq!(
        write(&mut slice, 95.0),
        40,
        "the first write checks every row"
    );

    // Row 7 spans 84 to 94 and is the only one that meets the edge between 90 and 95.
    assert_eq!(
        write(&mut slice, 90.0),
        1,
        "later writes check the rows the edge crosses"
    );
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_chunk_moved_by_whole_pixels_looks_up_no_glyph() {
    let mut slice = slice();
    let key = ChunkKey::new(1, 0);
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let ink = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let mut chunk = tx.record(key, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, ink);
    run(&mut chunk, font, "Glyphs", 14.0, [4.0, 20.0], fill);
    world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the run commits");
    assert!(slice.draw());

    let mut moved = |offset: [f32; 2]| {
        work(&mut slice, "work.glyph_chunks_prefetched", |slice| {
            let mut tx = slice.tx();
            tx.set_offset(key, offset);
            slice.compositor.commit(tx).expect("the offset commits");
            assert!(slice.draw());
        })
    };
    assert_eq!(moved([3.0, 5.0]), 0, "a whole-pixel move keeps every glyph");
    assert_eq!(
        moved([3.5, 5.0]),
        1,
        "a half-pixel move can need new glyphs"
    );
}

/// Records `count` rows of 8 pixels every 10 pixels, each a chunk of its own, in one order node.
fn list(slice: &mut Captured, count: u32) {
    list_in(slice, count, None);
}

/// Records the list `list` records, in a group with effect `effect` when one is given.
fn list_in(slice: &mut Captured, count: u32, effect: Option<EffectNode>) {
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    let node = tx.create_order(OrderNode::default());
    let mut entries = Vec::new();
    for row in 0..count {
        let key = ChunkKey::new(1, row);
        let mut chunk = tx.record(key, props(world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([4.0, row as f32 * 10.0, 80.0, 8.0], fill));
        entries.push(OrderEntry::Chunk(key));
    }
    tx.splice(node, 0, 0, &entries);
    let top = match effect {
        Some(effect) => {
            let effect = tx.create_effect(effect);
            let group = tx.create_order(OrderNode {
                effect: Some(effect),
            });
            tx.splice(group, 0, 0, &[OrderEntry::Node(node)]);
            group
        }
        None => node,
    };
    world.append(&mut tx, OrderEntry::Node(top));
    slice.compositor.commit(tx).expect("the list commits");
    assert!(slice.draw());
}

/// Records row 3 of the list `list` made again, filled with `paint`.
fn repaint_row(slice: &mut Captured, paint: iceblink::PaintId) {
    let mut tx = slice.tx();
    let mut chunk = tx.record(ChunkKey::new(1, 3), props(slice.world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([4.0, 30.0, 80.0, 8.0], fill));
    slice.compositor.commit(tx).expect("the row commits");
}

#[test]
fn a_chunk_recorded_again_over_the_same_ink_sums_no_order_node() {
    let mut slice = slice();
    list(&mut slice, 200);
    let mut tx = slice.tx();
    let other = slice.world.solid(&mut tx, [0.7, 0.2, 0.2, 1.0]);
    slice.compositor.commit(tx).expect("the paint commits");
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        repaint_row(slice, other);
        assert!(slice.draw());
    });
    assert_eq!(summed, 0, "no order node holding the row is summed again");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_frame_damaging_one_row_looks_only_at_the_rows_its_damage_meets() {
    let culled = |count: u32| {
        let mut slice = slice();
        list(&mut slice, count);
        let mut tx = slice.tx();
        let other = slice.world.solid(&mut tx, [0.7, 0.2, 0.2, 1.0]);
        slice.compositor.commit(tx).expect("the paint commits");
        repaint_row(&mut slice, other);
        assert!(slice.draw());
        slice.compositor.counters().frame.items_culled
    };
    assert_eq!(
        culled(400),
        culled(20),
        "rows outside the damage are not looked at"
    );
}

#[test]
fn a_scroll_port_that_changes_size_measures_no_carried_row_again() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let space = world.space;
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
    let port = tx.create_clip(square_in(space, [0.0, 0.0, 100.0, 100.0]));
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    for row in 0..30 {
        let key = ChunkKey::new(2, row);
        let mut row_props = props(content);
        row_props.clip = Some(port);
        let mut chunk = tx.record(key, row_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([4.0, row as f32 * 12.0, 80.0, 10.0], fill));
        world.append(&mut tx, OrderEntry::Chunk(key));
    }
    slice.compositor.commit(tx).expect("the rows commit");
    assert!(slice.draw());

    let measured = work(&mut slice, "work.inks_computed", |slice| {
        let mut tx = slice.tx();
        tx.set_clip(port, square_in(space, [0.0, 0.0, 100.0, 76.0]));
        slice.compositor.commit(tx).expect("the port commits");
        assert!(slice.draw());
    });
    assert_eq!(measured, 0, "the rows keep their inks in the content space");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_scroll_port_that_changes_size_sums_no_carried_order_node() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let space = world.space;
    let port_node = |width: f32| ScrollNode {
        space,
        port: [0.0, 0.0, width, 100.0],
        content: [width, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    };
    let scroll = tx.create_scroll(port_node(60.0));
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(space))
    });
    let port = tx.create_clip(square_in(space, [0.0, 0.0, 60.0, 100.0]));
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    let badge = ChunkKey::new(3, 0);
    let mut chunk = tx.record(badge, props(space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([100.0, 4.0, 8.0, 8.0], fill));
    world.append(&mut tx, OrderEntry::Chunk(badge));
    let scrolled = tx.create_order(OrderNode::default());
    let mut messages = Vec::new();
    for message in 0..30 {
        let node = tx.create_order(OrderNode::default());
        let mut entries = Vec::new();
        for line in 0..2 {
            let key = ChunkKey::new(2, message * 2 + line);
            let mut line_props = props(content);
            line_props.clip = Some(port);
            let mut chunk = tx.record(key, line_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, paint);
            let y = (message * 2 + line) as f32 * 6.0;
            chunk.quad(quad([4.0, y, 80.0, 5.0], fill));
            entries.push(OrderEntry::Chunk(key));
        }
        tx.splice(node, 0, 0, &entries);
        messages.push(OrderEntry::Node(node));
    }
    tx.splice(scrolled, 0, 0, &messages);
    world.append(&mut tx, OrderEntry::Node(scrolled));
    slice.compositor.commit(tx).expect("the messages commit");
    assert!(slice.draw());

    // A second badge goes in beside the port, so the root above the messages is summed before
    // they are.
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        tx.set_scroll_node(scroll, port_node(100.0));
        tx.set_clip(port, square_in(space, [0.0, 0.0, 100.0, 100.0]));
        let second = ChunkKey::new(3, 1);
        let mut chunk = tx.record(second, props(space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([100.0, 20.0, 8.0, 8.0], fill));
        slice.world.append(&mut tx, OrderEntry::Chunk(second));
        slice.compositor.commit(tx).expect("the port commits");
        assert!(slice.draw());
    });
    assert!(
        summed <= 3,
        "only the root, which no scroll node carries, is summed again: {summed}"
    );
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_scroll_port_that_grows_replaces_no_carry_and_widens_the_nodes_above_its_content() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let space = world.space;
    let port_node = |width: f32| ScrollNode {
        space,
        port: [0.0, 0.0, width, 100.0],
        content: [width, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    };
    let scroll = tx.create_scroll(port_node(60.0));
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(space))
    });
    let port = tx.create_clip(square_in(space, [0.0, 0.0, 60.0, 100.0]));
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    // The badge keeps the root on the device, so its ink follows the carried content's.
    let badge = ChunkKey::new(3, 0);
    let mut chunk = tx.record(badge, props(space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([0.0, 140.0, 8.0, 8.0], fill));
    world.append(&mut tx, OrderEntry::Chunk(badge));
    let scrolled = tx.create_order(OrderNode::default());
    let mut lines = Vec::new();
    for line in 0..20 {
        let node = tx.create_order(OrderNode::default());
        let key = ChunkKey::new(2, line);
        let mut line_props = props(content);
        line_props.clip = Some(port);
        let mut chunk = tx.record(key, line_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([4.0, line as f32 * 6.0, 80.0, 5.0], fill));
        tx.splice(node, 0, 0, &[OrderEntry::Chunk(key)]);
        lines.push(OrderEntry::Node(node));
    }
    tx.splice(scrolled, 0, 0, &lines);
    world.append(&mut tx, OrderEntry::Node(scrolled));
    slice.compositor.commit(tx).expect("the lines commit");
    assert!(slice.draw());

    let replaced = work(&mut slice, "work.carries_replaced", |slice| {
        let mut tx = slice.tx();
        tx.set_scroll_node(scroll, port_node(100.0));
        tx.set_clip(port, square_in(space, [0.0, 0.0, 100.0, 100.0]));
        slice.compositor.commit(tx).expect("the port commits");
        assert!(slice.draw());
    });
    assert_eq!(
        replaced, 0,
        "the carries refer to the port, whose pixels moved"
    );
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_scroll_port_that_grows_again_checks_only_its_moved_row_one_by_one() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let space = world.space;
    let port_node = |width: f32| ScrollNode {
        space,
        port: [0.0, 0.0, width, 100.0],
        content: [width, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    };
    let scroll = tx.create_scroll(port_node(60.0));
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(space))
    });
    // The first row's own space, which every resize moves.
    let nudged = |by: f32| TransformNode {
        local: iceblink::Matrix4::translation(by, 0.0, 0.0),
        ..identity(Some(content))
    };
    let row = tx.create_transform(nudged(0.0));
    let port = tx.create_clip(square_in(space, [0.0, 0.0, 60.0, 100.0]));
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    let scrolled = tx.create_order(OrderNode::default());
    let mut lines = Vec::new();
    for line in 0..40 {
        let key = ChunkKey::new(2, line);
        let mut line_props = props(if line == 0 { row } else { content });
        line_props.clip = Some(port);
        let mut chunk = tx.record(key, line_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([4.0, line as f32 * 6.0, 40.0, 5.0], fill));
        lines.push(OrderEntry::Chunk(key));
    }
    tx.splice(scrolled, 0, 0, &lines);
    world.append(&mut tx, OrderEntry::Node(scrolled));
    slice.compositor.commit(tx).expect("the lines commit");
    assert!(slice.draw());

    let resize = |slice: &mut Captured, width: f32| {
        let mut tx = slice.tx();
        tx.set_scroll_node(scroll, port_node(width));
        tx.set_clip(port, square_in(space, [0.0, 0.0, width, 100.0]));
        tx.set_transform_node(row, nudged(width / 20.0));
        slice.compositor.commit(tx).expect("the port commits");
        assert!(slice.draw());
    };
    resize(&mut slice, 80.0);
    let checked = work(&mut slice, "work.clip_users_checked", |slice| {
        resize(slice, 100.0);
    });
    assert_eq!(
        checked, 1,
        "the port keeps what restilling its other rows needs"
    );
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

/// Returns a clip node over `rect` in `space`, without radii.
fn square_in(space: iceblink::TransformId, rect: [f32; 4]) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii: [0.0; 8],
            shapes: ROUND,
        },
    }
}

#[test]
fn a_chunk_that_moves_inside_its_order_node_sums_no_order_node() {
    let mut slice = slice();
    list(&mut slice, 200);
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        tx.set_offset(ChunkKey::new(1, 3), [0.0, 1.0]);
        slice.compositor.commit(tx).expect("the offset commits");
        assert!(slice.draw());
    });
    assert_eq!(
        summed, 0,
        "the row stays inside the list's ink and in its row order"
    );
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);

    // Out of row order, the list and everything above it are summed again.
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        tx.set_offset(ChunkKey::new(1, 2), [0.0, 25.0]);
        slice.compositor.commit(tx).expect("the offset commits");
        assert!(slice.draw());
    });
    assert!(summed >= 200, "the list is summed again: {summed}");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_chunk_that_moves_off_the_edge_of_its_order_node_keeps_every_ink_exact() {
    let mut slice = slice();
    list(&mut slice, 8);
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        tx.set_offset(ChunkKey::new(1, 7), [0.0, -1.0]);
        slice.compositor.commit(tx).expect("the offset commits");
        assert!(slice.draw());
    });
    assert_eq!(summed, 0, "the last row stays inside the list's ink");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_chunk_that_moves_off_the_edge_of_an_isolated_group_sums_the_group() {
    let mut slice = slice();
    list_in(
        &mut slice,
        8,
        Some(EffectNode {
            clip: None,
            opacity: 1.0,
            blend: BlendMode::NORMAL,
            filters: Arc::from(Vec::new()),
            isolate: true,
        }),
    );
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        tx.set_offset(ChunkKey::new(1, 7), [0.0, -1.0]);
        slice.compositor.commit(tx).expect("the offset commits");
        assert!(slice.draw());
    });
    assert!(summed < 8, "the list keeps its ink: {summed}");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_chunk_that_grows_its_order_node_inside_the_node_above_keeps_every_ink_exact() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    let mut lists = Vec::new();
    for list in 0..3 {
        let key = ChunkKey::new(1, list);
        let mut chunk = tx.record(key, props(world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([4.0, list as f32 * 20.0, 80.0, 8.0], fill));
        let node = tx.create_order(OrderNode::default());
        tx.splice(node, 0, 0, &[OrderEntry::Chunk(key)]);
        lists.push(OrderEntry::Node(node));
    }
    let outer = tx.create_order(OrderNode::default());
    tx.splice(outer, 0, 0, &lists);
    world.append(&mut tx, OrderEntry::Node(outer));
    slice.compositor.commit(tx).expect("the lists commit");
    assert!(slice.draw());

    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        tx.set_offset(ChunkKey::new(1, 1), [0.0, 4.0]);
        slice.compositor.commit(tx).expect("the offset commits");
        assert!(slice.draw());
    });
    assert_eq!(
        summed, 0,
        "the middle list grows inside the outer list's ink"
    );
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_carried_row_that_moves_inside_its_list_sums_no_node_above_the_list() {
    let mut slice = slice();
    common::list::list(&mut slice, 20, false);
    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        tx.set_offset(ChunkKey::new(2, 3), [0.0, 1.0]);
        slice.compositor.commit(tx).expect("the offset commits");
        assert!(slice.draw());
    });
    assert_eq!(summed, 20, "the list's rows, and not the root's entries");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_chunk_moved_by_its_offset_counts_no_bytes_again() {
    let mut slice = slice();
    list(&mut slice, 20);
    let counted = work(&mut slice, "work.bytes_counted", |slice| {
        let mut tx = slice.tx();
        tx.set_offset(ChunkKey::new(1, 3), [0.0, 1.0]);
        slice.compositor.commit(tx).expect("the offset commits");
        assert!(slice.draw());
    });
    assert_eq!(counted, 0, "the recording and its lowering are the same");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_chunk_that_moves_out_of_row_order_above_a_carried_list_keeps_every_ink_exact() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    let scroll = tx.create_scroll(ScrollNode {
        space: world.space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [100.0, 100.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(world.space))
    });
    let list = tx.create_order(OrderNode::default());
    let mut rows = Vec::new();
    for row in 0..20 {
        let key = ChunkKey::new(4, row);
        let mut chunk = tx.record(key, props(content));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([0.0, row as f32 * 5.0, 100.0, 4.0], fill));
        rows.push(OrderEntry::Chunk(key));
    }
    tx.splice(list, 0, 0, &rows);
    let moved = ChunkKey::new(5, 0);
    let mut chunk = tx.record(moved, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([0.0, 100.0, 10.0, 4.0], fill));
    let panel = tx.create_order(OrderNode::default());
    tx.splice(
        panel,
        0,
        0,
        &[OrderEntry::Node(list), OrderEntry::Chunk(moved)],
    );
    let below = ChunkKey::new(5, 1);
    let mut chunk = tx.record(below, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([0.0, 110.0, 10.0, 4.0], fill));
    world.append(&mut tx, OrderEntry::Node(panel));
    world.append(&mut tx, OrderEntry::Chunk(below));
    slice.compositor.commit(tx).expect("the panel commits");
    assert!(slice.draw());

    // The panel grows past the chunk below it, out of the root's row order.
    let mut tx = slice.tx();
    tx.set_offset(moved, [0.0, 16.0]);
    slice.compositor.commit(tx).expect("the offset commits");
    assert!(slice.draw());
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn a_chunk_that_grows_past_its_order_node_grows_the_nodes_above_without_summing() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    let node = tx.create_order(OrderNode::default());
    let key = ChunkKey::new(3, 0);
    let mut chunk = tx.record(key, props(world.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([4.0, 4.0, 20.0, 8.0], fill));
    tx.splice(node, 0, 0, &[OrderEntry::Chunk(key)]);
    world.append(&mut tx, OrderEntry::Node(node));
    slice.compositor.commit(tx).expect("the bar commits");
    assert!(slice.draw());

    let summed = work(&mut slice, "work.entries_summed", |slice| {
        let mut tx = slice.tx();
        let mut chunk = tx.record(key, props(slice.world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([4.0, 4.0, 60.0, 8.0], fill));
        slice.compositor.commit(tx).expect("the longer bar commits");
        assert!(slice.draw());
    });
    assert_eq!(summed, 0, "the bar's node and the root grow by its new ink");
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn an_identical_recording_measures_no_ink() {
    let mut slice = slice();
    let key = ChunkKey::new(4, 0);
    let record = |slice: &mut Captured, paint: Option<iceblink::PaintId>| {
        let mut tx = slice.tx();
        let world = &mut slice.world;
        let paint = paint.unwrap_or_else(|| world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]));
        let mut chunk = tx.record(key, props(world.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([4.0, 4.0, 40.0, 8.0], fill));
        chunk.quad(quad([4.0, 16.0, 40.0, 8.0], fill));
        if world.entries == 0 {
            world.append(&mut tx, OrderEntry::Chunk(key));
        }
        slice.compositor.commit(tx).expect("the chunk commits");
        paint
    };
    let paint = record(&mut slice, None);
    assert!(slice.draw());
    let measured = work(&mut slice, "work.inks_computed", |slice| {
        record(slice, Some(paint));
        slice.draw();
    });
    assert_eq!(measured, 0, "the chunk keeps the inks it had");
    assert_eq!(slice.compositor.counters().commit.chunks_identical, 1);
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}

#[test]
fn glyphs_a_clip_cuts_away_are_left_out_of_their_run() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let ink = world.solid(&mut tx, [0.0, 0.0, 0.0, 1.0]);
    let font = world.font;
    let clip = tx.create_clip(square_in(world.space, [0.0, 0.0, 40.0, 128.0]));
    let mut cut = props(world.space);
    cut.clip = Some(clip);
    let key = ChunkKey::new(5, 0);
    let mut chunk = tx.record(key, cut);
    let fill = chunk.paint_ref(PaintKind::SOLID, ink);
    run(
        &mut chunk,
        font,
        "i i i i i i i i i i i i i i i i",
        14.0,
        [4.0, 20.0],
        fill,
    );
    world.append(&mut tx, OrderEntry::Chunk(key));
    slice.compositor.commit(tx).expect("the run commits");
    assert!(slice.draw());
    let drawn = slice.frame.lanes.mono.drawn();
    assert!(
        (1..16).contains(&drawn),
        "only the glyphs inside the clip are drawn, {drawn} of 16"
    );
}

#[test]
fn a_row_moved_to_the_end_and_recorded_again_keeps_every_ink_exact() {
    let mut slice = slice();
    let mut tx = slice.tx();
    let world = &mut slice.world;
    let paint = world.solid(&mut tx, [0.2, 0.2, 0.7, 1.0]);
    let scroll = tx.create_scroll(ScrollNode {
        space: world.space,
        port: [0.0, 0.0, 100.0, 60.0],
        content: [100.0, 1000.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(world.space))
    });
    let list = tx.create_order(OrderNode::default());
    // Each row is a background and a label in a node of its own, recorded at its first position.
    let record_label = |tx: &mut iceblink::Transaction, row: u32, at: u32| {
        let mut chunk = tx.record(ChunkKey::new(7, 2 * row + 1), props(content));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad(
            [4.0, at as f32 * 10.0 + 2.0, 20.0 + at as f32, 6.0],
            fill,
        ));
    };
    let mut rows = Vec::new();
    for row in 0..8 {
        let background = ChunkKey::new(7, 2 * row);
        let mut chunk = tx.record(background, props(content));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([0.0, row as f32 * 10.0, 100.0, 10.0], fill));
        record_label(&mut tx, row, row);
        let label = tx.create_order(OrderNode::default());
        tx.splice(
            label,
            0,
            0,
            &[OrderEntry::Chunk(ChunkKey::new(7, 2 * row + 1))],
        );
        let node = tx.create_order(OrderNode::default());
        tx.splice(
            node,
            0,
            0,
            &[OrderEntry::Chunk(background), OrderEntry::Node(label)],
        );
        rows.push(OrderEntry::Node(node));
    }
    tx.splice(list, 0, 0, &rows);
    world.append(&mut tx, OrderEntry::Node(list));
    slice.compositor.commit(tx).expect("the list commits");
    assert!(slice.draw());

    // The top row moves to the end, its background by its offset, and its label is recorded
    // again where it now lies.
    for step in 0..12_u32 {
        let row = step % 8;
        let at = step + 8;
        let mut tx = slice.tx();
        tx.splice(list, 0, 1, &[]);
        tx.splice(list, 7, 0, &[rows[row as usize]]);
        tx.set_offset(ChunkKey::new(7, 2 * row), [0.0, (at - row) as f32 * 10.0]);
        record_label(&mut tx, row, at);
        tx.scroll_to(scroll, [0.0, (step + 1) as f32 * 10.0]);
        slice.compositor.commit(tx).expect("the moved row commits");
        slice.draw();
    }
    assert_eq!(stat(&slice.compositor, "verification_mismatches"), 0);
}
