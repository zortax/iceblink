//! The cost of the frames a compositor fling moves, which depends on what the port shows and not
//! on how long the content is.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::{
    BufferAge, Built, ChunkKey, ClipNode, ClipShape, Color, FrameClock, FrameRequest, GesturePhase,
    OrderEntry, OrderNode, Paint, PaintKind, ROUND, ScrollBehaviour, ScrollGesture, ScrollNode,
    ScrollPhysics, ScrollSource, SnapKind, Sources, TransformNode,
};

use common::{FULL, Scene, identity, presented, props, quad, stat};

/// The refresh interval of the frames, in nanoseconds.
const FRAME: u64 = 16_666_667;

/// The height of a row.
const ROW: f32 = 15.0;

/// The cost of one frame of a fling.
#[derive(Debug, PartialEq, Eq)]
struct Cost {
    chunks_drawn: u32,
    items_drawn: u32,
    order_nodes_visited: u32,
    damage_px: u64,
    shifted_px: u64,
    shift_refusals: u32,
    inks_computed: u64,
    chunks_lowered: u64,
}

/// Flings a list of `rows` rows under a 200 by 200 port, and returns what each frame cost.
fn fling(rows: u32) -> Vec<Cost> {
    let mut scene = Scene::new();
    list(&mut scene, rows);
    flick(&mut scene);
    costs(&mut scene)
}

/// Commits a kinetic list of `rows` rows under a 200 by 200 port over a white background, and
/// draws a frame.
fn list(scene: &mut Scene, rows: u32) {
    let mut tx = scene.tx();
    let scroll = tx.create_scroll(ScrollNode {
        space: scene.space,
        port: [0.0, 0.0, 200.0, 200.0],
        content: [200.0, rows as f32 * ROW],
        offset: [0.0, 0.0],
        behaviour: Some(ScrollBehaviour {
            physics: ScrollPhysics::Kinetic,
            snap: Arc::from([]),
            snap_kind: SnapKind::Mandatory,
            bounce: Sources::NONE,
            user_scrollable: [false, true],
            chain: Sources::NONE,
        }),
    });
    let content = tx.create_transform(TransformNode {
        scroll: Some(scroll),
        ..identity(Some(scene.space))
    });
    let port = tx.create_clip(ClipNode {
        parent: None,
        space: scene.space,
        shape: ClipShape::RoundedRect {
            rect: [0.0, 0.0, 200.0, 200.0],
            radii: [0.0; 8],
            shapes: ROUND,
        },
    });

    let white = tx.create_paint(Paint::Solid(Color([1.0, 1.0, 1.0, 1.0])));
    let grey = tx.create_paint(Paint::Solid(Color([0.5, 0.5, 0.5, 1.0])));
    let background = ChunkKey::new(u64::MAX, 0);
    let mut chunk = tx.record(background, props(scene.space));
    let fill = chunk.paint_ref(PaintKind::SOLID, white);
    chunk.quad(quad([0.0, 0.0, 200.0, 200.0], fill));
    let list = tx.create_order(OrderNode::default());
    tx.splice(
        scene.root,
        0,
        0,
        &[OrderEntry::Chunk(background), OrderEntry::Node(list)],
    );

    let mut entries = Vec::new();
    for row in 0..rows {
        let key = ChunkKey::new(u64::from(row), 0);
        let mut row_props = props(content);
        row_props.clip = Some(port);
        let mut chunk = tx.record(key, row_props);
        let fill = chunk.paint_ref(PaintKind::SOLID, grey);
        chunk.quad(quad([0.0, row as f32 * ROW, 200.0, ROW - 2.0], fill));
        entries.push(OrderEntry::Chunk(key));
    }
    tx.splice(list, 0, 0, &entries);

    scene.commit(tx);
    scene.draw();
}

/// Hands the compositor a touchpad gesture that ends in a downward fling.
fn flick(scene: &mut Scene) {
    let gesture = |delta: f32, phase: GesturePhase| ScrollGesture {
        position: [100.0, 100.0],
        delta: [0.0, delta],
        phase,
        source: ScrollSource::Touchpad,
        time_ns: FRAME,
    };
    for (delta, phase) in [
        (0.0, GesturePhase::Begin),
        (20.0, GesturePhase::Update),
        (
            0.0,
            GesturePhase::End {
                velocity: [0.0, 3000.0],
            },
        ),
    ] {
        scene
            .compositor
            .scroll_input(scene.output, gesture(delta, phase));
    }
}

/// Builds and presents up to 98 frames of a fling, stopping at the first that builds nothing, and
/// returns what each cost.
fn costs(scene: &mut Scene) -> Vec<Cost> {
    let mut costs = Vec::new();
    for tick in 2..100 {
        let before = (
            stat(&scene.compositor, "work.inks_computed"),
            stat(&scene.compositor, "work.chunks_lowered"),
        );
        let request = FrameRequest {
            caps: &FULL,
            age: BufferAge::Kept,
            clock: FrameClock {
                now_ns: (tick - 1) * FRAME,
                present_ns: tick * FRAME,
                interval_ns: FRAME,
            },
            subpixel: None,
        };
        let built = scene
            .compositor
            .build_frame(scene.output, &request, &mut scene.frame);
        if built != Built::Frame {
            break;
        }

        let id = scene.frame.id;
        scene.compositor.frame_done(scene.output, id, &presented());
        let counters = scene.compositor.counters().frame;
        costs.push(Cost {
            chunks_drawn: counters.chunks_drawn,
            items_drawn: counters.items_drawn,
            order_nodes_visited: counters.order_nodes_visited,
            damage_px: counters.damage_px,
            shifted_px: counters.shifted_px,
            shift_refusals: counters.shift_refusals,
            inks_computed: stat(&scene.compositor, "work.inks_computed") - before.0,
            chunks_lowered: stat(&scene.compositor, "work.chunks_lowered") - before.1,
        });
    }
    costs
}

#[test]
fn a_fling_frame_costs_the_same_over_ten_times_the_content() {
    let short = fling(1_000);
    let long = fling(10_000);
    assert!(
        short.len() > 30,
        "the fling moves for {} frames",
        short.len()
    );
    assert_eq!(short, long);
    for cost in &short {
        assert_eq!(cost.shift_refusals, 0, "{cost:?}");
        assert!(cost.shifted_px > 0, "{cost:?}");
        assert_eq!(cost.inks_computed, 0, "{cost:?}");
    }
}
