//! Scrolls of content drawn through clips that differ from the scroll port, drawn into a kept
//! target on a device, against the same pictures redrawn whole.

mod common;

use std::sync::Arc;

use iceblink::{
    BlendMode, ChunkKey, ClipId, ClipNode, ClipShape, EffectNode, OrderEntry, OrderNode, PaintKind,
    ROUND, ScrollId, ScrollNode, Transaction, TransformId, TransformNode,
};
use iceblink_testing::gpu::device_lock;

use common::twin::Twin;
use common::{identity, props, quad};

/// The rectangle of the scroll port.
const PORT: [f32; 4] = [8.0, 8.0, 100.0, 100.0];

/// The ids the edits of a scroller write.
#[derive(Clone, Copy)]
struct Ids {
    scroll: ScrollId,
    clip: ClipId,
    space: TransformId,
}

/// Returns a clip node over `rect` with the same radius at every corner.
fn clip_node(space: TransformId, rect: [f32; 4], radius: f32) -> ClipNode {
    ClipNode {
        parent: None,
        space,
        shape: ClipShape::RoundedRect {
            rect,
            radii: [radius; 8],
            shapes: ROUND,
        },
    }
}

/// Builds an unclipped opaque backing over the output, and stripes in content scrolled through
/// `PORT` and drawn through a clip over `clip` with corners of `radius`.
///
/// With `around`, the stripes' order node has an effect that clips them to it without isolating.
fn scroller(twin: &mut Twin, clip: [f32; 4], radius: f32, around: Option<[f32; 4]>) -> Ids {
    twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        let output = [0.0, 0.0, 128.0, 128.0];
        world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
        let space = world.space;
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: PORT,
            content: [100.0, 400.0],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(scroll),
            ..identity(Some(space))
        });
        let clip = tx.create_clip(clip_node(space, clip, radius));
        let effect = around.map(|rect| {
            let clip = tx.create_clip(clip_node(space, rect, 0.0));
            tx.create_effect(EffectNode {
                clip: Some(clip),
                opacity: 1.0,
                blend: BlendMode::NORMAL,
                filters: Arc::from(Vec::new()),
                isolate: false,
            })
        });
        let list = tx.create_order(OrderNode { effect });
        world.append(tx, OrderEntry::Node(list));
        let dark = world.solid(tx, [0.1, 0.2, 0.6, 1.0]);
        for row in 0..20 {
            let key = ChunkKey::new(2, row);
            let mut chunk_props = props(content);
            chunk_props.clip = Some(clip);
            let mut chunk = tx.record(key, chunk_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, dark);
            chunk.quad(quad([0.0, row as f32 * 20.0, 128.0, 10.0], fill));
            tx.splice(list, row, 0, &[OrderEntry::Chunk(key)]);
        }
        Ids {
            scroll,
            clip,
            space,
        }
    })
}

/// An edit written into a transaction.
type Edit<'a> = &'a dyn Fn(&mut Transaction, Ids);

/// Commits each group of edits, one transaction per edit and one frame after each group, and
/// checks every frame against a whole redraw.
fn check(twin: &mut Twin, ids: Ids, groups: &[(&str, &[Edit<'_>])]) {
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");
    for (name, edits) in groups {
        for edit in *edits {
            twin.commit(&mut (), |tx, _, _| edit(tx, ids));
        }
        let found = twin.draw();
        assert_eq!(found.pixels, 0, "{name} differs: {found:?}");
    }
}

#[test]
fn a_scroll_under_a_clip_smaller_than_the_port_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let ids = scroller(&mut twin, [20.0, 20.0, 70.0, 70.0], 0.0, None);
    check(
        &mut twin,
        ids,
        &[
            (
                "a scroll",
                &[&|tx, ids| tx.scroll_to(ids.scroll, [0.0, 7.0])],
            ),
            (
                "a scroll back",
                &[&|tx, ids| tx.scroll_to(ids.scroll, [0.0, 2.0])],
            ),
        ],
    );
}

#[test]
fn a_scroll_under_a_rounded_port_clip_shifts_and_redraws_the_corners() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let ids = scroller(&mut twin, PORT, 8.0, None);
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");
    let port_area = 100 * 100;
    for (step, offset) in [7.0, 2.0, 13.0, 30.0, 29.0].into_iter().enumerate() {
        twin.commit(&mut (), |tx, _, _| tx.scroll_to(ids.scroll, [0.0, offset]));
        let found = twin.draw();
        assert_eq!(twin.kept.frame.shifts.len(), 1, "step {step} shifts");
        let damage = twin
            .kept
            .frame
            .damage
            .draw
            .area()
            .expect("the damage is not whole");
        assert!(
            damage < port_area / 2,
            "step {step} redraws {damage} pixels"
        );
        assert_eq!(found.pixels, 0, "step {step} differs: {found:?}");
    }
}

#[test]
fn a_scroll_under_a_rounded_port_and_a_hard_cut_inside_it_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let port = [8.0, 20.0, 110.0, 60.0];
    let scroll = twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        let output = [0.0, 0.0, 128.0, 128.0];
        world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
        let space = world.space;
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port,
            content: [110.0, 400.0],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(scroll),
            ..identity(Some(space))
        });
        let rounded = tx.create_clip(clip_node(space, port, 8.0));
        let cut = tx.create_clip(ClipNode {
            parent: Some(rounded),
            ..clip_node(space, [8.0, 20.0, 100.0, 60.0], 0.0)
        });
        let list = tx.create_order(OrderNode::default());
        world.append(tx, OrderEntry::Node(list));
        let dark = world.solid(tx, [0.1, 0.2, 0.6, 1.0]);
        for row in 0..20 {
            let key = ChunkKey::new(2, row);
            let mut chunk_props = props(content);
            chunk_props.clip = Some(cut);
            let mut chunk = tx.record(key, chunk_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, dark);
            chunk.quad(quad([0.0, 20.0 + row as f32 * 20.0, 128.0, 12.0], fill));
            tx.splice(list, row, 0, &[OrderEntry::Chunk(key)]);
        }
        scroll
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");
    for (step, offset) in [3.0, 6.0, 9.0, 12.0].into_iter().enumerate() {
        twin.commit(&mut (), |tx, _, _| tx.scroll_to(scroll, [0.0, offset]));
        let found = twin.draw();
        assert_eq!(found.pixels, 0, "step {step} differs: {found:?}");
    }
}

#[test]
fn a_scroll_under_an_effect_clip_smaller_than_the_port_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let ids = scroller(&mut twin, PORT, 0.0, Some([20.0, 20.0, 70.0, 70.0]));
    check(
        &mut twin,
        ids,
        &[
            (
                "a scroll",
                &[&|tx, ids| tx.scroll_to(ids.scroll, [0.0, 7.0])],
            ),
            (
                "a scroll back",
                &[&|tx, ids| tx.scroll_to(ids.scroll, [0.0, 2.0])],
            ),
        ],
    );
}

#[test]
fn scrolls_and_clip_writes_in_one_frame_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let ids = scroller(&mut twin, PORT, 0.0, None);
    let scroll = |offset: f32| {
        move |tx: &mut Transaction, ids: Ids| {
            tx.scroll_to(ids.scroll, [0.0, offset]);
        }
    };
    let cut = |rect: [f32; 4]| {
        move |tx: &mut Transaction, ids: Ids| {
            tx.set_clip(ids.clip, clip_node(ids.space, rect, 0.0));
        }
    };
    let (first, second, third, fourth) = (scroll(5.0), scroll(9.0), scroll(16.0), scroll(21.0));
    let shrunk = cut([8.0, 8.0, 100.0, 70.0]);
    let port = cut(PORT);
    check(
        &mut twin,
        ids,
        &[
            ("a scroll, then a shrink", &[&first, &shrunk]),
            ("a growth back to the port", &[&port]),
            ("a shrink, then a scroll", &[&shrunk, &second]),
            ("a growth back to the port, then a scroll", &[&port, &third]),
            ("a shrink alone", &[&shrunk]),
            ("a growth alone", &[&port]),
            ("a scroll alone", &[&fourth]),
        ],
    );
}

#[test]
fn a_scroll_of_unclipped_rows_under_an_effect_clip_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let scroll = twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        let output = [0.0, 0.0, 128.0, 128.0];
        world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
        let space = world.space;
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: PORT,
            content: [100.0, 400.0],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(scroll),
            ..identity(Some(space))
        });
        let clip = tx.create_clip(clip_node(space, [20.0, 20.0, 70.0, 70.0], 0.0));
        let effect = tx.create_effect(EffectNode {
            clip: Some(clip),
            opacity: 1.0,
            blend: BlendMode::NORMAL,
            filters: Arc::from(Vec::new()),
            isolate: false,
        });
        let list = tx.create_order(OrderNode {
            effect: Some(effect),
        });
        world.append(tx, OrderEntry::Node(list));
        let dark = world.solid(tx, [0.1, 0.2, 0.6, 1.0]);
        // Rows that stay inside the port, drawn through no clip of their own, so nothing carries
        // them.
        for row in 0..4 {
            let key = ChunkKey::new(2, row);
            let mut chunk = tx.record(key, props(content));
            let fill = chunk.paint_ref(PaintKind::SOLID, dark);
            chunk.quad(quad([10.0, 20.0 + row as f32 * 20.0, 96.0, 10.0], fill));
            tx.splice(list, row, 0, &[OrderEntry::Chunk(key)]);
        }
        scroll
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    twin.commit(&mut (), |tx, _, _| tx.scroll_to(scroll, [0.0, 7.0]));
    let found = twin.draw();
    assert_eq!(found.pixels, 0, "the scroll differs: {found:?}");
}

#[test]
fn a_scroll_inside_a_scrolled_view_shifts_and_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let inner = twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        let output = [0.0, 0.0, 128.0, 128.0];
        world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
        let space = world.space;
        let outer = tx.create_scroll(ScrollNode {
            space,
            port: output,
            content: [128.0, 400.0],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let page = tx.create_transform(TransformNode {
            scroll: Some(outer),
            ..identity(Some(space))
        });
        let inner = tx.create_scroll(ScrollNode {
            space: page,
            port: PORT,
            content: [100.0, 400.0],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(inner),
            ..identity(Some(page))
        });
        let clip = tx.create_clip(clip_node(page, PORT, 0.0));
        let list = tx.create_order(OrderNode::default());
        world.append(tx, OrderEntry::Node(list));
        let dark = world.solid(tx, [0.1, 0.2, 0.6, 1.0]);
        for row in 0..20 {
            let key = ChunkKey::new(2, row);
            let mut chunk_props = props(content);
            chunk_props.clip = Some(clip);
            let mut chunk = tx.record(key, chunk_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, dark);
            chunk.quad(quad([0.0, row as f32 * 20.0, 128.0, 10.0], fill));
            tx.splice(list, row, 0, &[OrderEntry::Chunk(key)]);
        }
        inner
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    twin.commit(&mut (), |tx, _, _| tx.scroll_to(inner, [0.0, 7.0]));
    let found = twin.draw();
    assert_eq!(twin.kept.frame.shifts.len(), 1, "the inner port shifts");
    assert_eq!(found.pixels, 0, "the scroll differs: {found:?}");
}

#[test]
fn a_scroll_of_rows_under_a_clip_that_moves_with_them_shifts() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let scroll = twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        let output = [0.0, 0.0, 128.0, 128.0];
        world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
        let space = world.space;
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: PORT,
            content: [100.0, 400.0],
            offset: [0.0, 0.0],
            behaviour: None,
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(scroll),
            ..identity(Some(space))
        });
        let port = tx.create_clip(clip_node(space, PORT, 0.0));
        let card = tx.create_clip(clip_node(content, [12.0, 0.0, 80.0, 400.0], 10.0));
        let effect = tx.create_effect(EffectNode {
            clip: Some(card),
            opacity: 1.0,
            blend: BlendMode::NORMAL,
            filters: Arc::from(Vec::new()),
            isolate: false,
        });
        let list = tx.create_order(OrderNode {
            effect: Some(effect),
        });
        world.append(tx, OrderEntry::Node(list));
        let dark = world.solid(tx, [0.1, 0.2, 0.6, 1.0]);
        for row in 0..20 {
            let key = ChunkKey::new(2, row);
            let mut chunk_props = props(content);
            chunk_props.clip = Some(port);
            let mut chunk = tx.record(key, chunk_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, dark);
            chunk.quad(quad([0.0, row as f32 * 20.0, 128.0, 10.0], fill));
            tx.splice(list, row, 0, &[OrderEntry::Chunk(key)]);
        }
        scroll
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    twin.commit(&mut (), |tx, _, _| tx.scroll_to(scroll, [0.0, 7.0]));
    let found = twin.draw();
    assert_eq!(twin.kept.frame.shifts.len(), 1, "the port shifts");
    assert_eq!(found.pixels, 0, "the scroll differs: {found:?}");
}
