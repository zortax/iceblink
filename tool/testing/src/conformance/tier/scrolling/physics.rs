//! Rules of scroll physics: instant deltas, flings, kinetic steps, snapping and returns from
//! overscroll.

use iceblink::{
    Built, CompositorEvent, GesturePhase, ScrollId, ScrollPhase, ScrollPhysics, SnapKind, Sources,
};

use crate::conformance::tier::FRAME_NS;
use crate::conformance::tier::scrollers::{
    DECAY, fling, fling_end, frames, list, offset_of, per_frame, snaps, vertical,
};
use crate::conformance::tier::stage::{Factory, Stage};

const MS: u64 = 1_000_000;

/// With instant physics a delta moves the node by the next frame, and the node settles once the
/// gesture ends.
pub fn instant_deltas_move_the_node_by_the_next_frame(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    stage.gesture([100.0, 100.0], [0.0, 0.0], GesturePhase::Begin, 0);
    stage.gesture([100.0, 100.0], [0.0, 25.0], GesturePhase::Update, MS);
    let events = frames(&mut stage, FRAME_NS, FRAME_NS);
    let (offset, _, _, phase) = Stage::last_scroll(&events, node).expect("the node moved");
    assert_eq!((offset[1], phase), (25.0, ScrollPhase::Gesture));

    stage.gesture(
        [100.0, 100.0],
        [0.0, 0.0],
        GesturePhase::End {
            velocity: [0.0, 900.0],
        },
        2 * FRAME_NS,
    );
    let events = frames(&mut stage, 2 * FRAME_NS, 4 * FRAME_NS);
    let (offset, _, _, phase) = Stage::last_scroll(&events, node).expect("the node settled");
    assert_eq!((offset[1], phase), (25.0, ScrollPhase::Settled));
}

/// A kinetic release at `v` pixels per second flings, where `r` is the share of the velocity left
/// after one second:
///
/// - `s` seconds on, the offset has moved `v (1 - r^s) / -ln r`.
/// - The fling ends in the first frame once it is slower than one pixel per refresh, `u`, having
///   moved `(v - u) / -ln r`.
pub fn a_fling_decays_by_the_formula(factory: Factory<'_>) {
    for interval in [FRAME_NS, 6_944_444] {
        let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
        let (mut stage, node) = list(factory, Some(behaviour));
        stage.interval_ns = interval;
        stage.draw_at(0);
        fling(&mut stage, 1000.0);
        let at = 6 * interval;
        let events = frames(&mut stage, interval, at);
        let seconds = at as f64 / 1e9;
        let expected = 10.0 + 1000.0 * (1.0 - DECAY.powf(seconds)) / -DECAY.ln();
        let offset = f64::from(offset_of(&events, node));
        assert!(
            (offset - expected).abs() <= 0.5,
            "{offset} is not {expected}"
        );

        let (distance, end_s) = fling_end(1000.0, interval);
        let frames = per_frame(&mut stage, at + interval, 3_000 * MS);
        let settled = frames
            .iter()
            .position(|events| {
                Stage::last_scroll(events, node)
                    .is_some_and(|(.., phase)| phase == ScrollPhase::Settled)
            })
            .expect("the fling settled");
        let settled_ns = at + (settled as u64 + 1) * interval;
        let end_ns = (end_s * 1e9) as u64;
        assert!(
            settled_ns >= end_ns && settled_ns < end_ns + interval,
            "at {interval} ns the fling settled at {settled_ns} ns, not in the frame after {end_ns} ns"
        );
        let (offset, ..) = Stage::last_scroll(&frames[settled], node).expect("the fling settled");
        let rest = 10.0 + distance;
        assert!(
            (f64::from(offset[1]) - rest).abs() <= 1.0,
            "at {interval} ns the fling rested at {offset:?}, not {rest}"
        );
    }
}

/// A kinetic step follows a 240 ms curve to its destination, and a further step during the
/// curve adds to the destination.
pub fn a_kinetic_step_follows_its_curve(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    stage.gesture([100.0, 100.0], [0.0, 100.0], GesturePhase::Step, 0);
    stage.draw_at(120 * MS);
    // `1 - (1 - p)³` at one half.
    let expected = 100.0 * 0.875;
    let offset = offset_of(&stage.events(), node);
    assert!(
        (offset - expected).abs() <= 0.5,
        "{offset} is not {expected}"
    );

    stage.gesture([100.0, 100.0], [0.0, 100.0], GesturePhase::Step, 120 * MS);
    let events = frames(&mut stage, 130 * MS, 600 * MS);
    assert_eq!(offset_of(&events, node), 200.0);
}

/// A step, and the curve of a kinetic step, move the node in `ScrollPhase::Step`.
pub fn steps_move_the_node_in_the_step_phase(factory: Factory<'_>) {
    for physics in [ScrollPhysics::Kinetic, ScrollPhysics::Instant] {
        let behaviour = vertical(physics, Sources::NONE, Sources::NONE);
        let (mut stage, node) = list(factory, Some(behaviour));
        stage.gesture([100.0, 100.0], [0.0, 100.0], GesturePhase::Step, 0);
        let events = frames(&mut stage, FRAME_NS, 600 * MS);
        let phases: Vec<ScrollPhase> = events
            .iter()
            .filter_map(|event| match *event {
                CompositorEvent::Scroll {
                    node: of, phase, ..
                } if of == node => Some(phase),
                _ => None,
            })
            .collect();
        let (last, moving) = phases.split_last().expect("the step moved the node");
        assert_eq!(*last, ScrollPhase::Settled, "{physics:?}");
        assert!(
            !moving.is_empty() && moving.iter().all(|phase| *phase == ScrollPhase::Step),
            "{physics:?} steps report {moving:?}"
        );
    }
}

/// When motion ends on a node with snap points, the node springs to the point nearest where the
/// motion would rest, starting at the release velocity. A proximity point too far from that
/// projection does not catch it.
pub fn motion_snaps_to_the_nearest_snap_point(factory: Factory<'_>) {
    let rest = 10.0 + fling_end(1000.0, FRAME_NS).0;
    for (kind, points, settled) in [
        (SnapKind::Mandatory, vec![0.0, 150.0, 300.0], 150.0),
        (SnapKind::Proximity, vec![0.0, 1000.0], rest),
    ] {
        let mut behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
        behaviour.snap_kind = kind;
        behaviour.snap = snaps(&points);
        let (mut stage, node) = list(factory, Some(behaviour));
        fling(&mut stage, 1000.0);
        let events = frames(&mut stage, FRAME_NS, 3_000 * MS);
        let (offset, _, _, phase) = Stage::last_scroll(&events, node).expect("the node settled");
        assert_eq!(phase, ScrollPhase::Settled);
        assert!(
            (f64::from(offset[1]) - settled).abs() <= 1.0,
            "{offset:?} is not {settled}"
        );
    }

    // A release too slow to fling projects to where it is, and the spring starts at its velocity.
    let mut behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    behaviour.snap = snaps(&[0.0, 150.0]);
    let (mut stage, node) = list(factory, Some(behaviour));
    let interval = 6_944_444;
    stage.interval_ns = interval;
    stage.draw_at(0);
    fling(&mut stage, 120.0);
    let first = frames(&mut stage, interval, interval);
    assert_eq!(offset_of(&first, node), 11.0, "the spring started at rest");
    let events = frames(&mut stage, 2 * interval, 3_000 * MS);
    let (offset, _, _, phase) = Stage::last_scroll(&events, node).expect("the node settled");
    assert_eq!((offset[1], phase), (0.0, ScrollPhase::Settled));
}

/// A displacement past the end returns once the finger lifts, and the node settles with none.
pub fn a_pull_past_the_end_returns(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::ALL, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    stage.gesture([100.0, 100.0], [0.0, 0.0], GesturePhase::Begin, 0);
    stage.gesture([100.0, 100.0], [0.0, -40.0], GesturePhase::Update, MS);
    let events = frames(&mut stage, FRAME_NS, FRAME_NS);
    let expected = -(200.0 * (1.0 - 1.0 / (0.55 * 40.0 / 200.0 + 1.0)));
    let (_, overscroll, _, _) = Stage::last_scroll(&events, node).expect("the content moved");
    assert!((overscroll[1] - expected).abs() <= 0.5, "{overscroll:?}");

    stage.gesture(
        [100.0, 100.0],
        [0.0, 0.0],
        GesturePhase::End {
            velocity: [0.0, 0.0],
        },
        2 * FRAME_NS,
    );
    let events = frames(&mut stage, 2 * FRAME_NS, 2_000 * MS);
    let (offset, overscroll, _, phase) = Stage::last_scroll(&events, node).expect("it returned");
    assert_eq!(
        (offset, overscroll, phase),
        ([0.0; 2], [0.0; 2], ScrollPhase::Settled)
    );
}

/// A node that a finger holds still wants no frame until the gesture's next step.
pub fn a_finger_holding_a_node_still_lets_the_output_sleep(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    stage.gesture([100.0, 100.0], [0.0, 0.0], GesturePhase::Begin, 0);
    stage.gesture([100.0, 100.0], [0.0, 25.0], GesturePhase::Update, MS);
    let events = frames(&mut stage, FRAME_NS, FRAME_NS);
    assert!(
        Stage::last_scroll(&events, node).is_some(),
        "the node moved"
    );

    assert_eq!(stage.draw_at(2 * FRAME_NS), Built::Idle { wake_ns: None });
}

/// A return and a snap end once they are within half a pixel of their target and slower than one
/// pixel per refresh, so the frame that first shows the target also settles the node.
pub fn returns_and_snaps_settle_in_the_frame_that_lands_them(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::ALL, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    stage.gesture([100.0, 100.0], [0.0, 0.0], GesturePhase::Begin, 0);
    stage.gesture([100.0, 100.0], [0.0, -40.0], GesturePhase::Update, MS);
    let lift = GesturePhase::End {
        velocity: [0.0, 0.0],
    };
    stage.gesture([100.0, 100.0], [0.0, 0.0], lift, 2 * MS);
    let frames = per_frame(&mut stage, FRAME_NS, 2_000 * MS);
    settles_where_it_lands(&frames, node, "the return", |_, overscroll| {
        overscroll == [0.0; 2]
    });

    let mut behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    behaviour.snap = snaps(&[0.0, 150.0, 300.0]);
    let (mut stage, node) = list(factory, Some(behaviour));
    fling(&mut stage, 1000.0);
    let frames = per_frame(&mut stage, FRAME_NS, 3_000 * MS);
    settles_where_it_lands(&frames, node, "the snap", |offset, _| offset[1] == 150.0);
}

/// Checks that the first of `frames` with a scroll event of `node` for which `landed` holds on
/// the offset and the overscroll is the frame that settles the node.
fn settles_where_it_lands(
    frames: &[Vec<CompositorEvent>],
    node: ScrollId,
    motion: &str,
    landed: impl Fn([f32; 2], [f32; 2]) -> bool,
) {
    let first = |holds: &dyn Fn([f32; 2], [f32; 2], ScrollPhase) -> bool| {
        frames.iter().position(|events| {
            events.iter().any(|event| match *event {
                CompositorEvent::Scroll {
                    node: of,
                    offset,
                    overscroll,
                    phase,
                    ..
                } => of == node && holds(offset, overscroll, phase),
                _ => false,
            })
        })
    };
    let lands = first(&|offset, overscroll, _| landed(offset, overscroll));
    let settles = first(&|_, _, phase| phase == ScrollPhase::Settled);
    assert!(lands.is_some(), "{motion} never landed");
    assert_eq!(
        settles, lands,
        "{motion} settled in frame {settles:?}, after it landed in frame {lands:?}"
    );
}
