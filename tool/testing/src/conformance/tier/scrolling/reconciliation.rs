//! Rules of reconciling compositor scrolling with the client's acknowledgements, writes and
//! animations.

use iceblink::smallvec::smallvec;
use iceblink::{
    Animation, AnimationEvent, AnimationStart, AnimationTarget, Built, CompositorEvent, Direction,
    Easing, FillMode, GesturePhase, Keyframe, Keyframes, ScrollId, ScrollNode, ScrollPhase,
    ScrollPhysics, Sources, Timing,
};

use crate::conformance::tier::FRAME_NS;
use crate::conformance::tier::scrollers::{fling, frames, list, offset_of, vertical};
use crate::conformance::tier::stage::{Factory, Stage};

const MS: u64 = 1_000_000;

/// Acknowledging a serial with the offset its event carried changes no pixel, and motion
/// carries on undisturbed.
pub fn acknowledging_a_serial_moves_no_pixel(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut quiet, node) = list(factory, Some(behaviour.clone()));
    fling(&mut quiet, 1000.0);
    let untouched = offset_of(&frames(&mut quiet, FRAME_NS, 3_000 * MS), node);

    let (mut stage, node) = list(factory, Some(behaviour));
    fling(&mut stage, 1000.0);
    let events = frames(&mut stage, FRAME_NS, 5 * FRAME_NS);
    let (offset, _, serial, _) = Stage::last_scroll(&events, node).expect("the fling moved");
    let mut tx = stage.tx();
    tx.acknowledge_scroll(node, serial, offset);
    stage.commit(tx);
    let events = frames(&mut stage, 6 * FRAME_NS, 3_000 * MS);
    let (offset, _, serial, _) = Stage::last_scroll(&events, node).expect("the fling settled");
    assert_eq!(offset[1], untouched);

    let mut tx = stage.tx();
    tx.acknowledge_scroll(node, serial, offset);
    stage.commit(tx);
    assert_eq!(stage.draw_at(3_100 * MS), Built::Idle { wake_ns: None });
}

/// An offset write ends the node's motion, and the written offset stays.
pub fn an_offset_write_ends_the_motion(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    fling(&mut stage, 1000.0);
    frames(&mut stage, FRAME_NS, 3 * FRAME_NS);
    let mut tx = stage.tx();
    tx.scroll_to(node, [0.0, 50.0]);
    stage.commit(tx);
    let events = frames(&mut stage, 4 * FRAME_NS, 1_000 * MS);
    assert!(Stage::last_scroll(&events, node).is_none(), "{events:?}");
    let hits = stage.compositor.hit_test(stage.output, [100.0, 0.0]);
    // Row 2 starts at 40 and holds the top edge at an offset of 50.
    assert_eq!(hits.first().map(|hit| hit.item), Some(2));
}

/// A node write whose offset equals the base offset keeps the compositor delta and the motion.
pub fn a_node_write_at_the_base_offset_keeps_the_motion(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour.clone()));
    fling(&mut stage, 1000.0);
    let before = offset_of(&frames(&mut stage, FRAME_NS, 3 * FRAME_NS), node);
    let mut tx = stage.tx();
    tx.set_scroll_node(
        node,
        ScrollNode {
            space: stage.space,
            port: [0.0, 0.0, 200.0, 200.0],
            content: [200.0, 4000.0],
            offset: [0.0; 2],
            behaviour: Some(behaviour),
        },
    );
    stage.commit(tx);
    let after = offset_of(&frames(&mut stage, 4 * FRAME_NS, 8 * FRAME_NS), node);
    assert!(after > before, "{after} after {before}");
}

/// An animation of a scroll offset moves the node programmatically, and a gesture that moves the
/// node ends the animation with `Replaced`, keeping the offset where it is.
pub fn a_gesture_ends_an_offset_animation(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Instant, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    let mut tx = stage.tx();
    let id = tx.animate(slide(node));
    stage.commit(tx);
    stage.draw_at(50 * MS);
    let (offset, _, _, phase) = Stage::last_scroll(&stage.events(), node).expect("it moved");
    assert_eq!((offset[1], phase), (50.0, ScrollPhase::Programmatic));

    stage.gesture([100.0, 100.0], [0.0, 5.0], GesturePhase::Begin, 50 * MS);
    let events = frames(&mut stage, 60 * MS, 60 * MS);
    assert!(events.iter().any(|event| matches!(
        event,
        CompositorEvent::Animation { id: of, event: AnimationEvent::Replaced, .. } if *of == id
    )));
    assert_eq!(offset_of(&events, node), 55.0);
}

/// Serials of a node's scroll events increase by one per event.
pub fn serials_increase_per_event(factory: Factory<'_>) {
    let behaviour = vertical(ScrollPhysics::Kinetic, Sources::NONE, Sources::NONE);
    let (mut stage, node) = list(factory, Some(behaviour));
    fling(&mut stage, 600.0);
    let serials: Vec<u64> = frames(&mut stage, FRAME_NS, 2_000 * MS)
        .iter()
        .filter_map(|event| match *event {
            CompositorEvent::Scroll {
                node: of, serial, ..
            } if of == node => Some(serial),
            _ => None,
        })
        .collect();
    assert!(serials.len() > 2);
    assert!(
        serials.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "{serials:?}"
    );
}

/// An offset animation that holds its end value lets the output sleep once it finished.
pub fn a_held_offset_animation_lets_the_output_sleep(factory: Factory<'_>) {
    let (mut stage, node) = list(factory, None);
    let mut tx = stage.tx();
    tx.animate(slide(node));
    stage.commit(tx);
    let events = frames(&mut stage, FRAME_NS, 300 * MS);
    assert_eq!(offset_of(&events, node), 100.0);
    assert_eq!(stage.draw_at(400 * MS), Built::Idle { wake_ns: None });
}

/// An animation of `node`'s offset from 0 to 100 down over 100 ms from time zero, which holds its
/// end value.
fn slide(node: ScrollId) -> Animation {
    Animation {
        target: AnimationTarget::ScrollOffset(node),
        keyframes: Keyframes::Vector(smallvec![
            Keyframe {
                offset: 0.0,
                value: [0.0, 0.0],
                easing: Easing::Linear,
            },
            Keyframe {
                offset: 1.0,
                value: [0.0, 100.0],
                easing: Easing::Linear,
            },
        ]),
        timing: Timing {
            duration_ns: 100 * MS,
            delay_ns: 0,
            iterations: 1.0,
            direction: Direction::Normal,
            fill: FillMode::Forwards,
            easing: Easing::Linear,
        },
        start: AnimationStart::At(0),
    }
}
