//! The damage each kind of change adds.

mod common;

use std::sync::Arc;

use iceblink::{
    BlendMode, Built, ChunkKey, ChunkProps, ClipNode, ClipShape, Color, EffectNode, ExternalQuad,
    FrameOutcome, Matrix4, OrderEntry, OrderNode, Paint, PaintKind, ROUND, ScrollNode, SpaceRef,
    TextureDesc, TransformNode,
};
use iceblink::{Compositor, OutputConfig};

use common::{Scene, identity, props, quad, rect};

/// Builds and presents a frame, and returns its draw damage sorted by row.
///
/// Panics if the damage is the whole output.
fn damage_of(scene: &mut Scene) -> Vec<iceblink::DeviceRect> {
    assert_eq!(scene.build(), Built::Frame);
    scene.done(common::presented());

    let mut rects = scene.damage().expect("the damage is not the whole output");
    rects.sort_by_key(|rect| (rect.origin.y, rect.origin.x));
    rects
}

fn covers(rects: &[iceblink::DeviceRect], expected: iceblink::DeviceRect) -> bool {
    let area: i64 = rects
        .iter()
        .filter_map(|held| held.intersection(expected))
        .map(|part| i64::from(part.size.width) * i64::from(part.size.height))
        .sum();
    area == i64::from(expected.size.width) * i64::from(expected.size.height)
}

fn area(rects: &[iceblink::DeviceRect]) -> i64 {
    rects
        .iter()
        .map(|rect| i64::from(rect.size.width) * i64::from(rect.size.height))
        .sum()
}

#[test]
fn a_new_chunk_damages_its_ink() {
    let mut scene = Scene::new();
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 20.0, 20.0]]);
    assert_eq!(damage_of(&mut scene), vec![rect(10, 10, 20, 20)]);
}

#[test]
fn removing_a_chunk_damages_its_old_ink() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0]]);
    scene.draw();
    let mut tx = scene.tx();
    tx.remove_chunk(key);
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(10, 10, 20, 20)]);
}

#[test]
fn an_offset_damages_old_and_new_ink() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0]]);
    scene.draw();
    let mut tx = scene.tx();
    tx.set_offset(key, [100.0, 0.0]);
    scene.commit(tx);
    let rects = damage_of(&mut scene);
    assert!(covers(&rects, rect(10, 10, 20, 20)));
    assert!(covers(&rects, rect(110, 10, 20, 20)));
    assert_eq!(area(&rects), 800);
}

#[test]
fn new_props_damage_old_and_new_ink() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0]]);
    let mut tx = scene.tx();
    let moved = tx.create_transform(TransformNode {
        local: Matrix4::translation(0.0, 100.0, 0.0),
        ..identity(None)
    });
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_props(key, props(moved));
    scene.commit(tx);
    let rects = damage_of(&mut scene);
    assert!(covers(&rects, rect(10, 10, 20, 20)));
    assert!(covers(&rects, rect(10, 110, 20, 20)));
}

#[test]
fn a_transform_write_damages_old_and_new_ink() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let moving = tx.create_transform(identity(None));
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(moving));
    chunk.quad(quad([0.0, 0.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_transform(moving, Matrix4::translation(50.0, 50.0, 0.0));
    scene.commit(tx);
    let rects = damage_of(&mut scene);
    assert!(covers(&rects, rect(0, 0, 10, 10)));
    assert!(covers(&rects, rect(50, 50, 10, 10)));
    assert_eq!(area(&rects), 200);
}

#[test]
fn a_paint_write_damages_the_items_naming_it() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let named = tx.create_paint(Paint::Solid(Color([0.0, 0.0, 1.0, 1.0])));
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, named);
    chunk.quad(quad([0.0, 0.0, 10.0, 10.0], fill));
    chunk.quad(quad([50.0, 50.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_paint(named, Paint::Solid(Color([0.0, 1.0, 0.0, 1.0])));
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(0, 0, 10, 10)]);
    assert!(
        !scene.frame.tables.paints.is_unchanged(),
        "the paint table carries the write"
    );
}

#[test]
fn a_splice_damages_inserted_and_removed_subtrees() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let child = tx.create_order(OrderNode::default());
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    chunk.quad(quad([20.0, 20.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(child, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    assert_eq!(
        scene.draw(),
        Built::Idle { wake_ns: None },
        "a subtree no output draws adds no damage"
    );

    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(child)]);
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(20, 20, 10, 10)]);

    let mut tx = scene.tx();
    tx.splice(scene.root, 0, 1, &[]);
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(20, 20, 10, 10)]);
}

#[test]
fn an_effect_write_damages_the_subtrees_naming_it() {
    let mut scene = Scene::new();
    let effect_node = |opacity: f32| EffectNode {
        clip: None,
        opacity,
        blend: BlendMode::NORMAL,
        filters: Arc::from(Vec::new()),
        isolate: false,
    };
    let mut tx = scene.tx();
    let effect = tx.create_effect(effect_node(1.0));
    let child = tx.create_order(OrderNode {
        effect: Some(effect),
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    chunk.quad(quad([20.0, 20.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(child, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(child)]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_effect(effect, effect_node(0.5));
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(20, 20, 10, 10)]);
    assert_eq!(scene.frame.groups.len(), 1, "an opacity below one isolates");
}

#[test]
fn a_root_write_damages_the_whole_output() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let other = tx.create_order(OrderNode::default());
    tx.set_root(scene.output, Some(other));
    scene.commit(tx);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.damage.draw.is_full());
}

#[test]
fn a_scroll_damages_old_and_new_ink_without_a_kept_target() {
    let mut scene = Scene::with_caps(iceblink::RenderCapabilities::MINIMAL);
    let mut tx = scene.tx();
    let scroll = tx.create_scroll(ScrollNode {
        space: scene.space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [100.0, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(scene.space))
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(content));
    chunk.quad(quad([0.0, 50.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.scroll_to(scroll, [0.0, 20.0]);
    scene.commit(tx);
    let rects = damage_of(&mut scene);
    assert!(covers(&rects, rect(0, 50, 10, 10)));
    assert!(covers(&rects, rect(0, 30, 10, 10)));
    assert!(scene.frame.shifts.is_empty());
}

/// Commits chunk 1 with an item at 20 and one at 70 in a scrolled list clipped by a port of 100
/// by 100, draws a frame, and returns the scroll node.
fn two_items(scene: &mut Scene) -> iceblink::ScrollId {
    let mut tx = scene.tx();
    let scroll = tx.create_scroll(ScrollNode {
        space: scene.space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [100.0, 400.0],
        offset: [0.0, 0.0],
        behaviour: None,
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(scene.space))
    });
    let port = tx.create_clip(ClipNode {
        parent: None,
        space: scene.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 100.0, 100.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });
    let mut row_props = props(content);
    row_props.clip = Some(port);
    record_two_items(&mut tx, row_props, 0.0);
    let list = tx.create_order(OrderNode::default());
    tx.splice(list, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(list)]);
    scene.commit(tx);
    scene.draw();
    scroll
}

/// Records chunk 1 with two squares of 10 at `x`, one at a `y` of 20 and one at 70.
fn record_two_items(tx: &mut iceblink::Transaction, chunk_props: ChunkProps, x: f32) {
    let mut chunk = tx.record(ChunkKey::new(1, 0), chunk_props);
    for y in [20.0, 70.0] {
        chunk.quad(quad([x, y, 10.0, 10.0], iceblink::PaintRef::NONE));
    }
}

#[test]
fn a_scrolled_list_without_a_kept_target_damages_its_items_and_not_the_gaps_between_them() {
    let mut scene = Scene::with_caps(iceblink::RenderCapabilities::MINIMAL);
    let scroll = two_items(&mut scene);

    let mut tx = scene.tx();
    tx.scroll_to(scroll, [0.0, 10.0]);
    scene.commit(tx);
    let rects = damage_of(&mut scene);
    assert!(covers(&rects, rect(0, 10, 10, 20)), "the upper item");
    assert!(covers(&rects, rect(0, 60, 10, 20)), "the lower item");
    assert!(
        rects
            .iter()
            .all(|held| !held.intersects(rect(0, 30, 10, 30))),
        "the gap between them: {rects:?}"
    );
}

#[test]
fn items_scrolled_and_then_moved_out_of_the_scroll_before_a_frame_are_damaged_where_it_drew_them() {
    let mut scene = Scene::with_caps(iceblink::RenderCapabilities::MINIMAL);
    let scroll = two_items(&mut scene);

    let mut tx = scene.tx();
    tx.scroll_to(scroll, [0.0, 10.0]);
    scene.commit(tx);
    let mut tx = scene.tx();
    let space = scene.space;
    record_two_items(&mut tx, props(space), 50.0);
    scene.commit(tx);
    let rects = damage_of(&mut scene);
    assert!(covers(&rects, rect(0, 20, 10, 10)), "the upper item");
    assert!(covers(&rects, rect(0, 70, 10, 10)), "the lower item");
}

#[test]
fn a_texture_frame_damages_its_external_quads() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let texture = tx.create_texture(TextureDesc {
        width: 8,
        height: 8,
        premultiplied: true,
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    let slot = chunk.texture_slot(texture);
    chunk.external(ExternalQuad {
        bounds: [30.0, 30.0, 8.0, 8.0],
        texture: slot,
        opacity: 1.0,
        clip: iceblink::ClipRef::INHERIT,
        space: SpaceRef::INHERIT,
    });
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    scene.draw();
    assert_eq!(scene.frame.externals.len(), 1);

    let mut tx = scene.tx();
    tx.texture_frame(texture);
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(30, 30, 8, 8)]);
}

#[test]
fn growing_an_output_damages_the_pixels_it_adds() {
    let mut scene = Scene::new();
    scene.compositor.configure_output(
        scene.output,
        OutputConfig {
            size: [220, 200],
            scale: 1.0,
            opaque: true,
        },
    );
    assert_eq!(damage_of(&mut scene), vec![rect(200, 0, 20, 200)]);
}

#[test]
fn new_capabilities_damage_the_whole_output() {
    let mut scene = Scene::new();
    scene.caps.custom_shaders = false;
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.damage.draw.is_full());
}

#[test]
fn a_recovered_device_damages_the_whole_next_frame() {
    let mut scene = Scene::new();
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 20.0, 20.0]]);
    assert_eq!(scene.build(), Built::Frame);
    scene.done(FrameOutcome::Recovered);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.damage.draw.is_full());
    assert!(
        scene.frame.tables.spaces.replace,
        "a new renderer gets every table"
    );
}

#[test]
fn an_unrecorded_frame_keeps_its_damage() {
    let mut scene = Scene::new();
    scene.add_chunk(ChunkKey::new(1, 0), &[[10.0, 10.0, 20.0, 20.0]]);
    assert_eq!(scene.build(), Built::Frame);
    scene.done(FrameOutcome::Skipped(iceblink::SkipReason::Unacquired));
    assert_eq!(damage_of(&mut scene), vec![rect(10, 10, 20, 20)]);
    assert_eq!(scene.build(), Built::Idle { wake_ns: None });
}

#[test]
fn an_order_effect_write_damages_the_subtree() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let effect = tx.create_effect(EffectNode {
        clip: None,
        opacity: 0.5,
        blend: BlendMode::NORMAL,
        filters: Arc::from(vec![iceblink::Filter::Blur(1.0)]),
        isolate: false,
    });
    let child = tx.create_order(OrderNode::default());
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(scene.space));
    chunk.quad(quad([40.0, 40.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(child, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    tx.splice(scene.root, 0, 0, &[OrderEntry::Node(child)]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_order_effect(child, Some(effect));
    scene.commit(tx);
    let reach = iceblink::read_extent([40.0, 40.0, 10.0, 10.0], &[iceblink::Filter::Blur(1.0)]);
    let region = rect(
        reach[0] as i32,
        reach[1] as i32,
        reach[2] as i32,
        reach[3] as i32,
    );
    let rects = damage_of(&mut scene);
    assert!(
        covers(&rects, region),
        "the group's written region is drawn"
    );
}

#[test]
fn a_transform_node_write_damages_old_and_new_ink() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let moving = tx.create_transform(identity(None));
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(moving));
    chunk.quad(quad([0.0, 0.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_transform_node(
        moving,
        TransformNode {
            local: Matrix4::scale(2.0, 2.0, 1.0),
            ..identity(Some(scene.space))
        },
    );
    scene.commit(tx);
    assert_eq!(damage_of(&mut scene), vec![rect(0, 0, 20, 20)]);
}

#[test]
fn a_scroll_node_write_damages_old_and_new_ink() {
    let mut scene = Scene::with_caps(iceblink::RenderCapabilities::MINIMAL);
    let node = |offset: f32| ScrollNode {
        space: scene.space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [100.0, 400.0],
        offset: [0.0, offset],
        behaviour: None,
    };
    let first = node(0.0);
    let second = node(30.0);
    let mut tx = scene.tx();
    let scroll = tx.create_scroll(first);
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(scene.space))
    });
    let mut chunk = tx.record(ChunkKey::new(1, 0), props(content));
    chunk.quad(quad([0.0, 50.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(1, 0))]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_scroll_node(scroll, second);
    scene.commit(tx);
    let rects = damage_of(&mut scene);
    assert!(covers(&rects, rect(0, 50, 10, 10)));
    assert!(covers(&rects, rect(0, 20, 10, 10)));
}

#[test]
fn a_recording_with_a_fractional_move_damages_old_and_new_ink() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let moving = tx.create_transform(identity(None));
    let key = ChunkKey::new(1, 0);
    let mut chunk = tx.record(key, props(moving));
    chunk.quad(quad([0.0, 0.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
    scene.commit(tx);
    scene.draw();

    let mut tx = scene.tx();
    tx.set_transform(moving, Matrix4::translation(0.5, 0.0, 0.0));
    let mut chunk = tx.record(key, props(moving));
    chunk.quad(quad([0.0, 0.0, 10.0, 10.0], iceblink::PaintRef::NONE));
    scene.commit(tx);
    assert!(covers(&damage_of(&mut scene), rect(0, 0, 11, 10)));
}

#[test]
fn recording_a_chunk_at_an_offset_again_damages_old_and_new_ink() {
    let mut scene = Scene::new();
    let key = ChunkKey::new(1, 0);
    let space = scene.space;
    let record = |scene: &mut Scene, place: bool| {
        let mut tx = scene.tx();
        let mut chunk = tx.record(key, props(space));
        chunk.quad(quad([10.0, 10.0, 20.0, 20.0], iceblink::PaintRef::NONE));
        if place {
            tx.splice(scene.root, 0, 0, &[OrderEntry::Chunk(key)]);
        }
        scene.commit(tx);
    };
    record(&mut scene, true);
    let mut tx = scene.tx();
    tx.set_offset(key, [0.3, 0.0]);
    scene.commit(tx);
    scene.draw();

    record(&mut scene, false);
    assert!(covers(&damage_of(&mut scene), rect(10, 10, 21, 20)));
}
