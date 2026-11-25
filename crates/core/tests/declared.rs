//! Damage a transaction declares, taken in place of the damage its changes derive.

mod common;

use iceblink::Compositor;
use iceblink::damage::DamageSet;
use std::sync::Arc;

use iceblink::{
    Built, ChunkKey, CompositorEvent, GesturePhase, ScrollBehaviour, ScrollGesture, ScrollNode,
    ScrollPhysics, ScrollSource, SnapKind, Sources,
};

use common::{Scene, rect};

/// Returns a damage set of `rects`.
fn set(rects: &[iceblink::DeviceRect]) -> DamageSet {
    let mut damage = DamageSet::new();
    for &held in rects {
        damage.absorb(held);
    }
    damage
}

/// Returns a scene with verification set to `verify`, past a frame that drew one chunk with a quad
/// over `[10, 10, 20, 20]`.
fn drawn(verify: bool) -> (Scene, ChunkKey) {
    let mut scene = Scene::new();
    scene.compositor.set_verification(verify);
    let key = ChunkKey::new(1, 0);
    scene.add_chunk(key, &[[10.0, 10.0, 20.0, 20.0]]);
    assert_eq!(scene.draw(), Built::Frame);
    (scene, key)
}

#[test]
fn a_declared_commit_damages_what_it_declares() {
    let (mut scene, key) = drawn(false);
    let mut tx = scene.tx();
    tx.set_offset(key, [40.0, 0.0]);
    tx.declare_damage(scene.output, set(&[rect(0, 0, 80, 40)]));
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert_eq!(scene.damage(), Some(vec![rect(0, 0, 80, 40)]));
}

#[test]
fn an_acknowledge_that_moves_no_pixel_keeps_the_declaration() {
    let (mut scene, key) = drawn(false);
    let mut tx = scene.tx();
    let scroll = tx.create_scroll(ScrollNode {
        space: scene.space,
        port: [0.0, 100.0, 100.0, 100.0],
        content: [100.0, 400.0],
        offset: [0.0; 2],
        behaviour: Some(ScrollBehaviour {
            physics: ScrollPhysics::Instant,
            snap: Arc::from([]),
            snap_kind: SnapKind::Mandatory,
            bounce: Sources::NONE,
            user_scrollable: [false, true],
            chain: Sources::NONE,
        }),
    });
    let content = tx.create_transform(iceblink::TransformNode {
        scroll: Some(scroll),
        ..common::identity(Some(scene.space))
    });
    let list = ChunkKey::new(2, 0);
    let paint = tx.create_paint(iceblink::Paint::Solid(iceblink::Color([
        0.0, 0.0, 1.0, 1.0,
    ])));
    let mut chunk = tx.record(list, common::props(content));
    let fill = chunk.paint_ref(iceblink::PaintKind::SOLID, paint);
    chunk.quad(common::quad([0.0, 100.0, 100.0, 400.0], fill));
    tx.splice(
        scene.root,
        scene.entries,
        0,
        &[iceblink::OrderEntry::Chunk(list)],
    );
    scene.entries += 1;
    scene.commit(tx);
    scene.draw();
    scene.compositor.scroll_input(
        scene.output,
        ScrollGesture {
            position: [50.0, 150.0],
            delta: [0.0, 30.0],
            phase: GesturePhase::Step,
            source: ScrollSource::Wheel,
            time_ns: 0,
        },
    );
    scene.draw();
    let mut events = Vec::new();
    scene.compositor.drain_events(&mut events);
    let (serial, offset) = events
        .iter()
        .rev()
        .find_map(|event| match *event {
            CompositorEvent::Scroll { serial, offset, .. } => Some((serial, offset)),
            _ => None,
        })
        .expect("the step moved the node");

    let mut tx = scene.tx();
    tx.acknowledge_scroll(scroll, serial, offset);
    tx.set_offset(key, [40.0, 0.0]);
    tx.declare_damage(scene.output, set(&[rect(0, 0, 80, 40)]));
    scene.commit(tx);
    assert_eq!(scene.draw(), Built::Frame);
    assert_eq!(scene.damage(), Some(vec![rect(0, 0, 80, 40)]));
}

#[test]
fn verification_reports_a_declaration_that_misses_derived_damage() {
    let (mut scene, key) = drawn(true);
    let mut tx = scene.tx();
    tx.set_offset(key, [40.0, 0.0]);
    tx.declare_damage(scene.output, set(&[rect(0, 0, 40, 40)]));
    scene.commit(tx);
    assert_eq!(scene.compositor.counters().damage_violations, 1);
    let mut events = Vec::new();
    scene.compositor.drain_events(&mut events);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, CompositorEvent::DamageUndeclared { .. })),
        "the violation is reported: {events:?}"
    );
    assert_eq!(scene.draw(), Built::Frame);
    let damage = scene.damage().expect("the damage is partial");
    let union = damage.iter().copied().reduce(|a, b| a.union(b));
    assert_eq!(
        union,
        Some(rect(10, 10, 60, 20)),
        "the derived damage is drawn in place of the declaration"
    );

    // A declaration that holds the derived damage passes.
    let mut tx = scene.tx();
    tx.set_offset(key, [0.0, 0.0]);
    tx.declare_damage(scene.output, set(&[rect(0, 0, 80, 40)]));
    scene.commit(tx);
    assert_eq!(scene.compositor.counters().damage_violations, 1);
}

#[test]
fn verification_holds_each_derived_rectangle_against_the_declaration() {
    let mut scene = Scene::new();
    let mut tx = scene.tx();
    let paint = tx.create_paint(iceblink::Paint::Solid(iceblink::Color([
        1.0, 0.0, 0.0, 1.0,
    ])));
    // Two squares at the ends of an L, and five along a diagonal far enough apart that the full
    // set merges the two squares, whose union covers the corner the L leaves out.
    let squares = [
        [18, 0],
        [0, 18],
        [40, 40],
        [60, 60],
        [80, 80],
        [100, 100],
        [120, 120],
    ];
    let mut entries = Vec::new();
    for (owner, [x, y]) in (1..).zip(squares) {
        let key = ChunkKey::new(owner, 0);
        let mut chunk = tx.record(key, common::props(scene.space));
        let fill = chunk.paint_ref(iceblink::PaintKind::SOLID, paint);
        chunk.quad(common::quad([x as f32, y as f32, 2.0, 2.0], fill));
        entries.push(iceblink::OrderEntry::Chunk(key));
    }
    tx.splice(scene.root, 0, 0, &entries);
    tx.declare_damage(
        scene.output,
        set(&[rect(0, 0, 20, 2), rect(0, 2, 2, 18), rect(40, 40, 82, 82)]),
    );
    scene.commit(tx);

    assert_eq!(scene.compositor.counters().damage_violations, 0);
    let mut events = Vec::new();
    scene.compositor.drain_events(&mut events);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, CompositorEvent::DamageUndeclared { .. })),
        "no change damages a pixel outside the declaration: {events:?}"
    );
}
