//! Scroll offset changes answered with shifts of the kept target, and the refusals.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::{
    BlendMode, Built, ChunkKey, ClipNode, ClipShape, Color, EffectNode, Filter, FrameOutcome,
    OrderEntry, OrderNode, Paint, PaintKind, PaintRef, ROUND, RenderCapabilities, ScrollId,
    ScrollNode, Shift, SkipReason, TransformNode,
};

use common::{Scene, identity, props, quad, rect, stat};

/// What a scroller draws besides its content.
#[derive(Clone, Copy, PartialEq)]
enum Over {
    Nothing,
    /// A quad after the content, inside the port.
    Quad,
    /// A blurred group after the content, whose ink misses the port and whose read extent
    /// reaches into it.
    Blur,
}

/// Returns a scene with a scroll port of 100 by 100 at the origin, clipped by a clip node, with
/// content of 100 by 400 under it, and the port's scroll node.
///
/// With `backed`, an opaque background chunk covers the output first.
fn scroller(caps: RenderCapabilities, backed: bool, over: Over) -> (Scene, ScrollId) {
    let mut scene = Scene::with_caps(caps);
    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
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

    let mut entries = Vec::new();
    if backed {
        let key = ChunkKey::new(1, 0);
        let mut chunk = tx.record(key, props(scene.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([0.0, 0.0, 200.0, 200.0], fill));
        entries.push(OrderEntry::Chunk(key));
    }
    let key = ChunkKey::new(2, 0);
    let mut content_props = props(content);
    content_props.clip = Some(port);
    let mut chunk = tx.record(key, content_props);
    for row in 0..8 {
        chunk.quad(quad([10.0, row as f32 * 50.0, 80.0, 30.0], PaintRef::NONE));
    }
    entries.push(OrderEntry::Chunk(key));
    if over == Over::Quad {
        let key = ChunkKey::new(3, 0);
        let mut chunk = tx.record(key, props(scene.space));
        chunk.quad(quad([40.0, 40.0, 10.0, 10.0], PaintRef::NONE));
        entries.push(OrderEntry::Chunk(key));
    }
    if over == Over::Blur {
        let effect = tx.create_effect(EffectNode {
            clip: None,
            opacity: 1.0,
            blend: BlendMode::NORMAL,
            filters: Arc::from(vec![Filter::Blur(4.0)]),
            isolate: false,
        });
        let group = tx.create_order(OrderNode {
            effect: Some(effect),
        });
        let key = ChunkKey::new(4, 0);
        let mut chunk = tx.record(key, props(scene.space));
        chunk.quad(quad([104.0, 40.0, 10.0, 10.0], PaintRef::NONE));
        tx.splice(group, 0, 0, &[OrderEntry::Chunk(key)]);
        entries.push(OrderEntry::Node(group));
    }
    tx.splice(scene.root, 0, 0, &entries);
    scene.commit(tx);
    scene.draw();
    (scene, scroll)
}

fn scroll_by(scene: &mut Scene, scroll: ScrollId, offset: f32) {
    let mut tx = scene.tx();
    tx.scroll_to(scroll, [0.0, offset]);
    scene.commit(tx);
}

#[test]
fn a_backed_port_shifts_and_redraws_the_exposed_band() {
    let (mut scene, scroll) = scroller(common::FULL, true, Over::Nothing);
    scroll_by(&mut scene, scroll, 20.0);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(
        scene.frame.shifts.as_slice(),
        &[Shift {
            region: rect(0, 0, 100, 100),
            by: [0, -20],
        }]
    );
    assert_eq!(scene.damage(), Some(vec![rect(0, 80, 100, 20)]));
    let present = scene.frame.damage.present;
    assert!(
        present.intersects(rect(0, 0, 100, 80)),
        "the moved pixels are presented"
    );
    let frame = scene.compositor.counters().frame;
    assert_eq!(frame.shifted_px, 100 * 80);
    assert_eq!(frame.shift_refusals, 0);
}

#[test]
fn a_port_without_an_opaque_backing_is_refused() {
    let (mut scene, scroll) = scroller(common::FULL, false, Over::Nothing);
    scroll_by(&mut scene, scroll, 20.0);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.shifts.is_empty());
    assert_eq!(scene.compositor.counters().frame.shift_refusals, 1);
}

#[test]
fn a_port_drawn_over_is_refused() {
    let (mut scene, scroll) = scroller(common::FULL, true, Over::Quad);
    scroll_by(&mut scene, scroll, 20.0);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.shifts.is_empty());
    assert_eq!(scene.compositor.counters().frame.shift_refusals, 1);
}

#[test]
fn a_refusal_is_counted_under_the_condition_it_failed() {
    let cases = [
        (false, Over::Nothing, "shift.refused.not_backed"),
        (true, Over::Quad, "shift.refused.overdrawn"),
        (true, Over::Blur, "shift.refused.read"),
    ];
    for (backed, over, reason) in cases {
        let (mut scene, scroll) = scroller(common::FULL, backed, over);
        scroll_by(&mut scene, scroll, 20.0);
        assert_eq!(scene.build(), Built::Frame);
        assert_eq!(stat(&scene.compositor, reason), 1, "{reason}");
    }
}

#[test]
fn a_renderer_that_cannot_shift_gets_damage() {
    let (mut scene, scroll) = scroller(RenderCapabilities::MINIMAL, true, Over::Nothing);
    scroll_by(&mut scene, scroll, 20.0);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.shifts.is_empty());
    assert_eq!(scene.compositor.counters().frame.shift_refusals, 0);
}

#[test]
fn no_shift_builds_on_a_frame_that_kept_its_damage() {
    let (mut scene, scroll) = scroller(common::FULL, true, Over::Nothing);
    scroll_by(&mut scene, scroll, 10.0);
    assert_eq!(scene.build(), Built::Frame);
    scene.done(FrameOutcome::Skipped(SkipReason::Unacquired));
    scroll_by(&mut scene, scroll, 20.0);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.shifts.is_empty());
}

#[test]
fn a_port_a_blur_reads_into_is_refused() {
    let (mut scene, scroll) = scroller(common::FULL, true, Over::Blur);
    scroll_by(&mut scene, scroll, 20.0);
    assert_eq!(scene.build(), Built::Frame);
    assert!(scene.frame.shifts.is_empty());
    assert_eq!(scene.compositor.counters().frame.shift_refusals, 1);
}

#[test]
fn a_list_of_many_chunks_shifts() {
    let mut scene = Scene::with_caps(common::FULL);
    let mut tx = scene.tx();
    let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
    let rows = 200;
    let scroll = tx.create_scroll(ScrollNode {
        space: scene.space,
        port: [0.0, 0.0, 100.0, 100.0],
        content: [100.0, rows as f32 * 10.0],
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
    let background = ChunkKey::new(1, 0);
    let mut chunk = tx.record(background, props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, paint);
    chunk.quad(quad([0.0, 0.0, 200.0, 200.0], fill));
    let list = tx.create_order(OrderNode { effect: None });
    let mut entries = Vec::new();
    for row in 0..rows {
        let key = ChunkKey::new(2, row);
        let mut row_props = props(content);
        row_props.clip = Some(port);
        let mut chunk = tx.record(key, row_props);
        chunk.quad(quad([10.0, row as f32 * 10.0, 80.0, 6.0], PaintRef::NONE));
        entries.push(OrderEntry::Chunk(key));
    }
    tx.splice(list, 0, 0, &entries);
    tx.splice(
        scene.root,
        0,
        0,
        &[OrderEntry::Chunk(background), OrderEntry::Node(list)],
    );
    scene.commit(tx);
    scene.draw();

    scroll_by(&mut scene, scroll, 20.0);
    assert_eq!(scene.build(), Built::Frame);
    assert_eq!(
        scene.frame.shifts.as_slice(),
        &[Shift {
            region: rect(0, 0, 100, 100),
            by: [0, -20],
        }]
    );
}

/// A backed scroll port of 100 by 100 at the origin, as `scroller` makes it, with the ids that
/// record its content again.
struct Rerecorded {
    scene: Scene,
    scroll: ScrollId,
    content: iceblink::TransformId,
    port: iceblink::ClipId,
}

impl Rerecorded {
    fn new() -> Self {
        let mut scene = Scene::with_caps(common::FULL);
        let mut tx = scene.tx();
        let paint = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
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

        let background = ChunkKey::new(1, 0);
        let mut chunk = tx.record(background, props(scene.space));
        let fill = chunk.paint_ref(PaintKind::SOLID, paint);
        chunk.quad(quad([0.0, 0.0, 200.0, 200.0], fill));
        tx.splice(
            scene.root,
            0,
            0,
            &[
                OrderEntry::Chunk(background),
                OrderEntry::Chunk(ChunkKey::new(2, 0)),
            ],
        );

        let mut rows = Self {
            scene,
            scroll,
            content,
            port,
        };
        rows.record(&mut tx, &Self::rows());
        rows.scene.commit(tx);
        rows.scene.draw();
        rows
    }

    /// Returns the bounds of the eight rows the content starts with.
    fn rows() -> Vec<[f32; 4]> {
        (0..8)
            .map(|row| [10.0, row as f32 * 50.0, 80.0, 30.0])
            .collect()
    }

    /// Records the content as one quad per rectangle of `rows`.
    fn record(&self, tx: &mut iceblink::Transaction, rows: &[[f32; 4]]) {
        let mut content_props = props(self.content);
        content_props.clip = Some(self.port);
        let mut chunk = tx.record(ChunkKey::new(2, 0), content_props);
        for &bounds in rows {
            chunk.quad(quad(bounds, PaintRef::NONE));
        }
    }

    /// Scrolls by `offset` and records the content as `rows` in one transaction.
    fn scroll_and_record(&mut self, offset: f32, rows: &[[f32; 4]]) {
        let mut tx = self.scene.tx();
        tx.scroll_to(self.scroll, [0.0, offset]);
        self.record(&mut tx, rows);
        self.scene.commit(tx);
    }
}

#[test]
fn a_scroll_committed_with_an_identical_recording_shifts_as_a_plain_scroll() {
    let mut rows = Rerecorded::new();
    rows.scroll_and_record(20.0, &Rerecorded::rows());
    assert_eq!(rows.scene.build(), Built::Frame);
    assert_eq!(
        rows.scene.frame.shifts.as_slice(),
        &[Shift {
            region: rect(0, 0, 100, 100),
            by: [0, -20],
        }]
    );
    assert_eq!(rows.scene.damage(), Some(vec![rect(0, 80, 100, 20)]));
}

#[test]
fn a_scroll_committed_with_a_changed_recording_damages_the_change_where_it_is() {
    let mut rows = Rerecorded::new();
    let mut changed = Rerecorded::rows();
    changed[1] = [20.0, 50.0, 60.0, 30.0];
    changed.push([10.0, 60.0, 5.0, 5.0]);
    rows.scroll_and_record(20.0, &changed);
    assert_eq!(rows.scene.build(), Built::Frame);
    assert_eq!(rows.scene.frame.shifts.len(), 1);
    let damage = rows
        .scene
        .damage()
        .expect("the damage is not the whole output");
    let covers = |expected: iceblink::DeviceRect| {
        damage
            .iter()
            .filter_map(|held| held.intersection(expected))
            .map(|part| part.size.width * part.size.height)
            .sum::<i32>()
            == expected.size.width * expected.size.height
    };
    assert!(covers(rect(20, 30, 60, 30)), "the changed row where it is");
    assert!(covers(rect(10, 40, 5, 5)), "the added quad where it is");
}
