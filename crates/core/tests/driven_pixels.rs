//! Compositor-driven motion drawn into a kept target on a device, compared with the same motion
//! redrawn whole.

mod common;

use std::sync::Arc;

use iceblink::Compositor;
use iceblink::smallvec::smallvec;
use iceblink::{
    Animation, AnimationStart, AnimationTarget, BlendMode, ChunkKey, ClipNode, ClipShape,
    Direction, Easing, EffectNode, FillMode, GesturePhase, Keyframe, Keyframes, Matrix4,
    OrderEntry, OrderNode, PaintKind, ROUND, ScrollBehaviour, ScrollGesture, ScrollId, ScrollNode,
    ScrollPhysics, ScrollSource, SnapKind, Sources, Timing, TransformNode,
};
use iceblink_testing::gpu::device_lock;

use common::twin::Twin;
use common::{identity, props, quad};

const MS: u64 = 1_000_000;

/// The refresh interval of the frames drawn.
const FRAME: u64 = 16_666_667;

/// The scroll port.
const PORT: [f32; 4] = [8.0, 8.0, 100.0, 100.0];

/// Commits an opaque backing over the output and stripes drawn through a clip of `PORT`, and
/// returns the scroll node.
///
/// The stripes scroll through the port by a kinetic node that bounces.
fn scroller(twin: &mut Twin) -> ScrollId {
    twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        world.boxes(
            tx,
            ChunkKey::new(1, 0),
            white,
            &[[0.0, 0.0, 128.0, 128.0]],
            true,
        );
        let space = world.space;
        let scroll = tx.create_scroll(ScrollNode {
            space,
            port: PORT,
            content: [100.0, 600.0],
            offset: [0.0, 0.0],
            behaviour: Some(ScrollBehaviour {
                physics: ScrollPhysics::Kinetic,
                snap: Arc::from([]),
                snap_kind: SnapKind::Mandatory,
                bounce: Sources::ALL,
                user_scrollable: [false, true],
                chain: Sources::NONE,
            }),
        });
        let content = tx.create_transform(TransformNode {
            scroll: Some(scroll),
            ..identity(Some(space))
        });
        let clip = tx.create_clip(ClipNode {
            parent: None,
            space,
            shape: ClipShape::RoundedRect {
                rect: PORT,
                radii: [0.0; 8],
                shapes: ROUND,
            },
        });
        let list = tx.create_order(OrderNode::default());
        world.append(tx, OrderEntry::Node(list));
        let dark = world.solid(tx, [0.1, 0.2, 0.6, 1.0]);
        for row in 0..30 {
            let key = ChunkKey::new(2, row);
            let mut chunk_props = props(content);
            chunk_props.clip = Some(clip);
            let mut chunk = tx.record(key, chunk_props);
            let fill = chunk.paint_ref(PaintKind::SOLID, dark);
            chunk.quad(quad([8.0, 8.0 + row as f32 * 20.0, 100.0, 10.0], fill));
            tx.splice(list, row, 0, &[OrderEntry::Chunk(key)]);
        }
        scroll
    })
}

fn gesture(delta: [f32; 2], phase: GesturePhase, time_ns: u64) -> ScrollGesture {
    ScrollGesture {
        position: [50.0, 50.0],
        delta,
        phase,
        source: ScrollSource::Touchpad,
        time_ns,
    }
}

/// Draws frames from `from_ns` to `to_ns` and checks each against a whole redraw.
///
/// Returns the most pixels a kept frame shifted.
fn frames(twin: &mut Twin, name: &str, from_ns: u64, to_ns: u64) -> u64 {
    let mut shifted = 0;
    let mut at = from_ns;
    while at <= to_ns {
        twin.at(at, FRAME);
        let found = twin.draw();
        assert_eq!(
            found.pixels,
            0,
            "{name} at {} ms differs: {found:?}",
            at / MS
        );
        shifted = shifted.max(twin.kept.compositor.counters().frame.shifted_px);
        at += FRAME;
    }
    shifted
}

#[test]
fn a_fling_and_a_bounce_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    scroller(&mut twin);
    twin.at(0, FRAME);
    assert_eq!(twin.draw().pixels, 0, "the first frames agree");

    twin.input(gesture([0.0, 0.0], GesturePhase::Begin, 0));
    for step in 1..4 {
        twin.input(gesture([0.0, 12.0], GesturePhase::Update, step * 8 * MS));
    }
    twin.input(gesture(
        [0.0, 0.0],
        GesturePhase::End {
            velocity: [0.0, 1800.0],
        },
        32 * MS,
    ));
    let shifted = frames(&mut twin, "the fling", FRAME, 1_500 * MS);
    assert!(shifted > 0, "the fling moves the port by shifts");

    let start = 1_600 * MS;
    twin.input(gesture([0.0, 0.0], GesturePhase::Begin, start));
    twin.input(gesture([0.0, -600.0], GesturePhase::Update, start + 8 * MS));
    twin.input(gesture(
        [0.0, 0.0],
        GesturePhase::End {
            velocity: [0.0, 0.0],
        },
        start + 16 * MS,
    ));
    frames(
        &mut twin,
        "the fling back",
        start + FRAME,
        start + 2_000 * MS,
    );
}

#[test]
fn animated_opacity_and_transforms_draw_what_a_whole_redraw_draws() {
    let _device = device_lock();
    let Some(mut twin) = Twin::new() else {
        return;
    };
    let timing = Timing {
        duration_ns: 300 * MS,
        delay_ns: 0,
        iterations: 1.0,
        direction: Direction::Alternate,
        fill: FillMode::Forwards,
        easing: Easing::CubicBezier([0.42, 0.0, 0.58, 1.0]),
    };
    twin.commit(&mut (), |tx, world, _| {
        let white = world.solid(tx, [1.0, 1.0, 1.0, 1.0]);
        world.boxes(
            tx,
            ChunkKey::new(1, 0),
            white,
            &[[0.0, 0.0, 128.0, 128.0]],
            true,
        );
        let effect = tx.create_effect(EffectNode {
            clip: None,
            opacity: 1.0,
            blend: BlendMode::NORMAL,
            filters: Arc::from([]),
            isolate: false,
        });
        let spun = tx.create_transform(identity(Some(world.space)));
        let group = tx.create_order(OrderNode {
            effect: Some(effect),
        });
        world.append(tx, OrderEntry::Node(group));
        let red = world.solid(tx, [0.9, 0.1, 0.1, 1.0]);
        let mut chunk = tx.record(ChunkKey::new(2, 0), props(spun));
        let fill = chunk.paint_ref(PaintKind::SOLID, red);
        chunk.quad(quad([-20.0, -20.0, 40.0, 40.0], fill));
        tx.splice(group, 0, 0, &[OrderEntry::Chunk(ChunkKey::new(2, 0))]);

        tx.animate(Animation {
            target: AnimationTarget::Opacity(effect),
            keyframes: Keyframes::Scalar(smallvec![
                Keyframe {
                    offset: 0.0,
                    value: 1.0,
                    easing: Easing::Linear
                },
                Keyframe {
                    offset: 1.0,
                    value: 0.25,
                    easing: Easing::Linear
                },
            ]),
            timing: timing.clone(),
            start: AnimationStart::At(0),
        });
        let (sin, cos) = 1.2_f32.sin_cos();
        let turned = Matrix4::from_columns([
            [cos, sin, 0.0, 0.0],
            [-sin, cos, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        tx.animate(Animation {
            target: AnimationTarget::Transform(spun),
            keyframes: Keyframes::Matrix(smallvec![
                Keyframe {
                    offset: 0.0,
                    value: Matrix4::translation(30.0, 30.0, 0.0),
                    easing: Easing::Linear,
                },
                Keyframe {
                    offset: 1.0,
                    value: turned.then(&Matrix4::translation(90.0, 80.0, 0.0)),
                    easing: Easing::Linear,
                },
            ]),
            timing: timing.clone(),
            start: AnimationStart::At(0),
        });
    });
    frames(&mut twin, "the animation", 0, 400 * MS);
}
