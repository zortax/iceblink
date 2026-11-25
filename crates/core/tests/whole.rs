//! Frames drawn into a kept target on a device, against the same pictures redrawn whole.

mod common;

use std::sync::Arc;

use iceblink::{
    BlendMode, ChunkKey, ClipNode, ClipShape, EffectNode, Filter, Matrix4, OrderEntry, OrderId,
    OrderNode, PaintKind, ROUND, ScrollNode, Transaction, TransformId, TransformNode,
};
use iceblink_testing::gpu::device_lock;

use common::story::World;
use common::twin::Twin;
use common::{identity, props, quad};

/// Records an opaque white backing over the output and appends it to the root.
fn backing(tx: &mut Transaction, world: &mut World) {
    let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
    let output = [0.0, 0.0, 128.0, 128.0];
    world.boxes(tx, ChunkKey::new(1, 0), white, &[output], true);
}

/// Creates a group blurred with a deviation of 2, appends it to the root and returns it.
fn blurred(tx: &mut Transaction, world: &mut World) -> OrderId {
    let blur = tx.create_effect(EffectNode {
        clip: None,
        opacity: 1.0,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![Filter::Blur(2.0)]),
        isolate: true,
    });
    let group = tx.create_order(OrderNode { effect: Some(blur) });
    world.append(tx, OrderEntry::Node(group));
    group
}

/// Records chunk `key` with one dark quad over `bounds` in `space`, drawn through `clip`, and
/// appends it to `parent`.
fn row(
    tx: &mut Transaction,
    world: &mut World,
    (key, parent): (ChunkKey, OrderId),
    space: TransformId,
    clip: Option<iceblink::ClipId>,
    bounds: [f32; 4],
) {
    let dark = world.solid(tx, [0.1, 0.2, 0.6, 1.0]);
    let mut chunk_props = props(space);
    chunk_props.clip = clip;
    let mut chunk = tx.record(key, chunk_props);
    let fill = chunk.paint_ref(PaintKind::SOLID, dark);
    chunk.quad(quad(bounds, fill));
    tx.splice(parent, 0, 0, &[OrderEntry::Chunk(key)]);
}

#[test]
fn content_moved_inside_a_blurred_group_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let space = twin.commit(&mut (), |tx, world, _| {
        backing(tx, world);
        let group = blurred(tx, world);
        let space = tx.create_transform(identity(Some(world.space)));
        let bounds = [30.0, 30.0, 40.0, 20.0];
        row(tx, world, (ChunkKey::new(2, 0), group), space, None, bounds);
        space
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    twin.commit(&mut (), |tx, _, _| {
        tx.set_transform(space, Matrix4::translation(20.0, 0.0, 0.0));
    });
    let found = twin.draw();
    assert_eq!(found.pixels, 0, "the moved group differs: {found:?}");
}

#[test]
fn a_scroll_that_moves_blurred_content_out_of_its_port_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let scroll = twin.commit(&mut (), |tx, world, _| {
        backing(tx, world);
        let space = world.space;
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: [0.0, 0.0, 100.0, 100.0],
            content: [100.0, 400.0],
            offset: [0.0, 20.0],
            behaviour: None,
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(scroll),
            ..identity(Some(space))
        });
        let port = tx.create_clip(ClipNode {
            parent: None,
            space,
            shape: ClipShape::RoundedRect {
                rect: [0.0, 0.0, 100.0, 100.0],
                radii: [0.0; 8],
                shapes: ROUND,
            },
        });
        let list = tx.create_order(OrderNode::default());
        world.append(tx, OrderEntry::Node(list));
        let group = blurred(tx, world);
        let rows = [
            (ChunkKey::new(2, 0), list, [10.0, 30.0, 60.0, 10.0]),
            // Drawn at the bottom of the port, and below it once the scroll moves it down.
            (ChunkKey::new(2, 1), group, [10.0, 112.0, 60.0, 8.0]),
        ];
        for (key, parent, bounds) in rows {
            row(tx, world, (key, parent), content, Some(port), bounds);
        }
        scroll
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    twin.commit(&mut (), |tx, _, _| tx.scroll_to(scroll, [0.0, 10.0]));
    let found = twin.draw();
    assert_eq!(found.pixels, 0, "the scrolled port differs: {found:?}");
}

#[test]
fn a_scroll_that_shifts_a_faded_group_with_its_content_draws_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let scroll = twin.commit(&mut (), |tx, world, _| {
        backing(tx, world);
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
        let port = tx.create_clip(ClipNode {
            parent: None,
            space,
            shape: ClipShape::RoundedRect {
                rect: [0.0, 0.0, 100.0, 100.0],
                radii: [0.0; 8],
                shapes: ROUND,
            },
        });
        let faded = tx.create_effect(EffectNode {
            clip: Some(port),
            opacity: 0.5,
            blend: BlendMode::NORMAL,
            filters: Arc::from(Vec::new()),
            isolate: false,
        });
        let list = tx.create_order(OrderNode::default());
        world.append(tx, OrderEntry::Node(list));
        let group = tx.create_order(OrderNode {
            effect: Some(faded),
        });
        tx.splice(list, 0, 0, &[OrderEntry::Node(group)]);
        let rows = [
            (ChunkKey::new(2, 0), list, [10.0, 30.0, 60.0, 10.0]),
            // Across the bottom edge of the port, and drawn in the band the scroll exposes.
            (ChunkKey::new(2, 1), group, [20.0, 84.0, 60.0, 30.0]),
        ];
        for (key, parent, bounds) in rows {
            row(tx, world, (key, parent), content, Some(port), bounds);
        }
        scroll
    });
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    twin.commit(&mut (), |tx, _, _| tx.scroll_to(scroll, [0.0, 10.0]));
    let found = twin.draw();
    assert_eq!(twin.kept.frame.shifts.len(), 1, "the kept slice shifts");
    assert_eq!(found.pixels, 0, "the scrolled port differs: {found:?}");
}
