//! The frame, vector-plan and mark-plan invariants, over random emission streams.

mod common;

use iceblink::damage::DamageSet;
use iceblink::{DeviceRect, FRAME_NO_CLIP};
use iceblink_core::plan::LaneKind;
use proptest::prelude::*;

use crate::common::plan::scene::{Scene, lane_sources};
use crate::common::plan::{invariants, rect};

#[derive(Clone, Debug)]
enum Op {
    Quad(DeviceRect, bool),
    Instance(LaneKind, DeviceRect, [u32; 2]),
    Mark(DeviceRect, bool),
    Path(DeviceRect, usize),
    External(DeviceRect),
    Backdrop(DeviceRect, bool),
    Begin(DeviceRect),
    End,
    Block(Vec<DeviceRect>),
}

fn area() -> impl Strategy<Value = DeviceRect> {
    (-40i32..440, -40i32..440, 1i32..160, 1i32..160)
        .prop_map(|(x, y, width, height)| rect(x, y, width, height))
}

fn lane() -> impl Strategy<Value = LaneKind> {
    prop::sample::select(vec![
        LaneKind::Shadows,
        LaneKind::ShaderQuads,
        LaneKind::Lines,
        LaneKind::Mono,
        LaneKind::Subpixel,
        LaneKind::Color,
    ])
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (area(), any::<bool>()).prop_map(|(ink, cover)| Op::Quad(ink, cover)),
        3 => (lane(), area(), 0u32..3, 0u32..2).prop_map(|(lane, ink, a, b)| Op::Instance(lane, ink, [a, b])),
        2 => (area(), any::<bool>()).prop_map(|(ink, union)| Op::Mark(ink, union)),
        3 => (area(), 0usize..5).prop_map(|(ink, clip)| Op::Path(ink, clip)),
        1 => area().prop_map(Op::External),
        1 => (area(), any::<bool>()).prop_map(|(ink, wide)| Op::Backdrop(ink, wide)),
        1 => area().prop_map(Op::Begin),
        1 => Just(Op::End),
        1 => prop::collection::vec(area(), 1..6).prop_map(Op::Block),
    ]
}

fn damage() -> impl Strategy<Value = DamageSet> {
    prop_oneof![
        Just(DamageSet::full()),
        Just(DamageSet::new()),
        prop::collection::vec(area(), 1..4).prop_map(|rects| {
            let mut damage = DamageSet::new();
            for piece in rects {
                damage.absorb(piece);
            }
            damage
        }),
    ]
}

/// Builds the scene `ops` describe, with balanced groups.
fn build(ops: &[Op], backdrops: bool) -> Scene {
    let mut scene = Scene::sized([400, 400]);
    let outer = scene.clips.only();
    let inner = scene.clips.push(outer);
    let beside = scene.clips.push(outer);
    let masked = scene.clips.mask(outer);
    let clips = [FRAME_NO_CLIP, outer, inner, beside, masked];

    let mut depth = 0;
    for op in ops {
        match op {
            Op::Quad(ink, true) => scene.cover(*ink, rect(ink.left() + 1, ink.top() + 1, 1, 1)),
            Op::Quad(ink, false) => scene.quad(*ink),
            Op::Instance(lane, ink, key) => scene.keyed(*lane, *ink, *key),
            Op::Mark(ink, union) => scene.mark(*ink, *union),
            Op::Path(ink, clip) => {
                scene.path(*ink, clips[*clip]);
            }
            Op::External(ink) => scene.external(*ink),
            Op::Backdrop(ink, wide) => {
                if backdrops {
                    scene.backdrop(*ink, *wide);
                }
            }
            Op::Begin(bounds) => {
                scene.begin_group(*bounds);
                depth += 1;
            }
            Op::End => {
                if depth > 0 {
                    scene.end_group();
                    depth -= 1;
                }
            }
            Op::Block(inks) => scene.block(LaneKind::Mono, inks),
        }
    }

    for _ in 0..depth {
        scene.end_group();
    }
    scene
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn every_plan_keeps_the_invariants(
        ops in prop::collection::vec(op(), 0..60),
        damage in damage(),
        backdrops in any::<bool>(),
    ) {
        let mut scene = build(&ops, backdrops);
        let planned = scene.plan(&damage);
        invariants::check(&scene, &damage, &planned);
    }

    #[test]
    fn a_planner_reused_across_frames_plans_what_a_fresh_one_does(
        first in prop::collection::vec(op(), 0..40),
        second in prop::collection::vec(op(), 0..40),
        damage in damage(),
    ) {
        use iceblink_core::plan::{Input, Output, Planner};
        use iceblink::Frame;

        let mut warm = Planner::new();
        let mut frame = Frame::default();
        let mut scene = build(&first, true);
        let input = Input::new(&scene.emission, &damage, scene.size);
        warm.plan(&input, &mut scene.clips, Output::of(&mut frame));

        let mut scene = build(&second, true);
        let input = Input::new(&scene.emission, &damage, scene.size);
        let stats = warm.plan(&input, &mut scene.clips, Output::of(&mut frame));
        let fresh = scene.plan(&damage);
        prop_assert_eq!(stats, fresh.stats);
        prop_assert_eq!(&frame.batches, &fresh.frame.batches);
        prop_assert_eq!(&frame.vector_plan, &fresh.frame.vector_plan);
        prop_assert_eq!(&frame.mark_plan, &fresh.frame.mark_plan);
        prop_assert_eq!(&frame.opaque_covers, &fresh.frame.opaque_covers);
        for lane in LaneKind::ALL {
            prop_assert_eq!(lane_sources(&frame, lane), lane_sources(&fresh.frame, lane));
        }
    }
}
